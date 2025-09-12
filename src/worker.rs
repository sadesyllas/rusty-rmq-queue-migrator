use crate::error::AppError;
use crate::rabbitmq::api::QueueInfo;
use futures_util::stream::{FuturesUnordered, StreamExt};
use indicatif::ProgressBar;
use lapin::options::{
    BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions,
    BasicQosOptions, ConfirmSelectOptions,
};
use lapin::Connection;
use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::task::JoinHandle;
use uuid::Uuid;

async fn consume_and_republish_loop(
    source_conn: Arc<Connection>,
    dest_conn: Arc<Connection>,
    queue_name: String,
    pb: Arc<ProgressBar>,
    consume_timeout: u64,
    prefetch_count: u16,
) -> Result<(), AppError> {
    let source_channel = source_conn.create_channel().await?;
    let dest_channel = dest_conn.create_channel().await?;

    dest_channel
        .confirm_select(ConfirmSelectOptions::default())
        .await?;

    source_channel
        .basic_qos(prefetch_count, BasicQosOptions::default())
        .await?;

    let consumer_tag = format!("migrator-{}-{}", queue_name, &Uuid::new_v4().to_string()[..8]);
    let mut consumer = source_channel
        .basic_consume(
            &queue_name,
            &consumer_tag,
            BasicConsumeOptions {
                no_ack: false, // We need to manually acknowledge messages.
                ..Default::default()
            },
            Default::default(),
        )
        .await?;

    loop {
        let delivery = match tokio::time::timeout(
            std::time::Duration::from_secs(consume_timeout),
            consumer.next(),
        )
        .await
        {
            Ok(Some(Ok(delivery))) => delivery,
            Ok(Some(Err(e))) => {
                error!("[{}] Error on consumer '{}': {}.", queue_name, consumer_tag, e);
                break;
            }
            Ok(None) => {
                info!("[{}] Consumer '{}' stream ended.", queue_name, consumer_tag);
                break;
            }
            Err(_) => {
                info!(
                    "[{}] Consumer '{}' timed out. Assuming queue is empty.",
                    queue_name, consumer_tag
                );
                break;
            }
        };

        debug!("[{}] Received message.", queue_name);

        let confirmation = dest_channel
            .basic_publish(
                "", // Default exchange
                &queue_name,
                BasicPublishOptions::default(),
                &delivery.data,
                delivery.properties.clone(),
            )
            .await?
            .await;

        match confirmation {
            Ok(_) => {
                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                    error!("[{}] Failed to ACK message: {}. This may result in a duplicate message.", queue_name, e);
                } else {
                    pb.inc(1);
                }
            }
            Err(e) => {
                warn!("[{}] Failed to publish message: {}. NACKing and requeueing.", queue_name, e);
                if let Err(nack_err) = delivery.nack(BasicNackOptions { requeue: true, ..Default::default() }).await {
                    error!("[{}] FATAL: Failed to NACK message after publish failure: {}. Message may be lost.", queue_name, nack_err);
                }
            }
        }
    }

    source_channel.close(200, "work complete").await?;
    dest_channel.close(200, "work complete").await?;

    Ok(())
}

/// Drains a single queue by consuming messages and republishing them to the destination.
pub async fn drain_queue(
    queue_info: QueueInfo,
    source_conn: Arc<Connection>,
    dest_conn: Arc<Connection>,
    pb: ProgressBar,
    consume_timeout: u64,
    prefetch_count: u16,
    queue_parallelism: u16,
) -> Result<(), AppError> {
    let queue_name = queue_info.name;
    let messages_to_drain = queue_info.messages_ready;

    info!(
        "[{}] Starting to drain queue with ~{} messages using {} parallel consumers.",
        queue_name, messages_to_drain, queue_parallelism
    );

    if messages_to_drain == 0 {
        return Ok(());
    }

    if queue_parallelism == 0 {
        warn!("[{}] `queue_parallelism` is 0, no workers will be started for this queue.", queue_name);
        return Ok(());
    }

    let pb = Arc::new(pb);
    let mut worker_handles = FuturesUnordered::new();

    for i in 0..queue_parallelism {
        let handle: JoinHandle<Result<(), AppError>> =
            tokio::spawn(consume_and_republish_loop(
                source_conn.clone(),
                dest_conn.clone(),
                queue_name.clone(),
                pb.clone(),
                consume_timeout,
                prefetch_count,
            ));
        worker_handles.push(handle);
        info!("[{}] Started consumer #{}", queue_name, i + 1);
    }

    while let Some(result) = worker_handles.next().await {
        match result {
            Ok(Ok(_)) => { /* Task completed successfully, continue looping */ }
            Ok(Err(app_err)) => return Err(app_err), // Task returned an error, so we exit
            Err(join_err) => return Err(AppError::WorkerJoinError(join_err)), // Task panicked or was cancelled
        }
    }

    info!("[{}] All consumers for queue have finished.", queue_name);
    Ok(())
}
