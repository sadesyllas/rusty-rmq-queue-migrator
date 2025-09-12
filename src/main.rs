use rusty_rmq_queue_migrator::{
    config::Config,
    error::AppError,
    rabbitmq::{api::ManagementApiClient, connector},
    worker,
};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{error, info, warn};
use rpassword;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

#[tokio::main]
async fn main() -> Result<()> {
    // region: --- Phase 1: Initialization & Validation ---

    let mut config = Config::parse();

    if config.source_password.is_none() {
        config.source_password =
            Some(rpassword::prompt_password("Enter source RabbitMQ password: ")?);
    }
    if config.dest_password.is_none() {
        config.dest_password =
            Some(rpassword::prompt_password("Enter destination RabbitMQ password: ")?);
    }

    setup_logging(&config)?;
    info!("Initialization complete. Configuration loaded and validated.");
    info!("Source: {}:{}", config.source_host, config.source_port);
    info!("Destination: {}:{}", config.dest_host, config.dest_port);

    if !config.only_preflight
        && config.source_host == config.dest_host
        && config.source_port == config.dest_port
        && config.source_vhost == config.dest_vhost
    {
        return Err(anyhow!(
            "Source and destination are the same. Migration would result in a no-op loop. Aborting."
        ));
    }

    let source_conn = Arc::new(
        connector::connect(
            &config.source_host,
            config.source_port,
            &config.source_username,
            config.source_password.as_deref().unwrap(),
            &config.source_vhost,
        )
        .await
        .context("Failed to connect to source RabbitMQ cluster")?,
    );
    info!(
        "Successfully connected to source cluster, vhost: {}",
        config.source_vhost
    );

    let dest_conn = Arc::new(
        connector::connect(
            &config.dest_host,
            config.dest_port,
            &config.dest_username,
            config.dest_password.as_deref().unwrap(),
            &config.dest_vhost,
        )
        .await
        .context("Failed to connect to destination RabbitMQ cluster")?,
    );
    info!("Successfully connected to destination cluster.");

    // endregion: --- Phase 1: Initialization & Validation ---

    // region: --- Phase 2: Pre-Flight Check & Discovery ---

    info!("Starting pre-flight check...");
    let mgmt_client = ManagementApiClient::new(&config);

    info!("Discovering queues with pending messages on source cluster...");
    let source_queues = mgmt_client
        .get_queues(
            config.queue_regex_filter.as_deref(),
            config.api_retries,
            config.api_retry_delay_secs,
        )
        .await?;

    let queues_to_migrate: Vec<_> = source_queues
        .into_iter()
        .filter(|q| q.messages_ready > 0)
        .collect();

    if config.queue_regex_filter.is_some() {
        info!(
            "Found {} queues matching the regex with ready messages.",
            queues_to_migrate.len()
        );
    } else {
        info!(
            "Found {} queues with ready messages.",
            queues_to_migrate.len()
        );
    }

    if queues_to_migrate.is_empty() {
        info!("No queues matched the criteria for migration. Exiting.");
        return Ok(());
    }

    info!("Verifying existence of destination queues...");
    let dest_channel = dest_conn.create_channel().await?;
    let mut missing_queues = Vec::new();
    for queue in &queues_to_migrate {
        if dest_channel
            .queue_declare(
                &queue.name,
                lapin::options::QueueDeclareOptions {
                    passive: true, // Don't create the queue, just check if it exists
                    ..Default::default()
                },
                Default::default(),
            )
            .await
            .is_err()
        {
            missing_queues.push(queue.name.clone());
        }
    }

    if !missing_queues.is_empty() {
        error!("The following destination queues are missing:");
        for q_name in missing_queues {
            error!("- {}", q_name);
        }
        return Err(anyhow!("Destination queues missing. Aborting to prevent data loss."));
    }
    info!("All destination queues verified.");
    dest_channel.close(200, "channel closed").await?;

    if queues_to_migrate.is_empty() {
        info!("No queues to migrate. Exiting.");
        return Ok(());
    }

    let total_messages: u64 = queues_to_migrate.iter().map(|q| q.messages_ready).sum();
    println!("--- Pre-flight Check Complete ---");
    println!(
        "Found {} queues to migrate with a total of {} messages:",
        queues_to_migrate.len(),
        total_messages
    );
    for q in &queues_to_migrate {
        println!("- {} ({} messages)", q.name, q.messages_ready);
    }

    if config.only_preflight {
        info!("Pre-flight check successful. Exiting due to --only-preflight flag.");
        return Ok(());
    }

    println!("\nPress Enter to begin migration or Ctrl+C to abort...");
    std::io::stdin().read_line(&mut String::new())?;

    // endregion: --- Phase 2: Pre-Flight Check & Discovery ---

    // region: --- Phase 3: Message Migration ---

    let start_time = Instant::now();
    info!("Starting message migration...");

    let num_workers = config.max_parallel_workers.unwrap_or_else(|| {
        let count = std::thread::available_parallelism().map_or(1, |p| p.get());
        info!(
            "`max_parallel_workers` not specified, defaulting to number of logical CPUs: {}",
            count
        );
        count
    });
    info!("Using {} parallel workers.", num_workers);

    let multi_progress = Arc::new(MultiProgress::new());
    let semaphore = Arc::new(Semaphore::new(num_workers));
    let mut worker_handles: Vec<JoinHandle<Result<(), AppError>>> = Vec::new();

    for queue_info in queues_to_migrate {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let source_conn = source_conn.clone();
        let dest_conn = dest_conn.clone();
        let mp = multi_progress.clone();
        let consume_timeout = config.consume_timeout;
        let prefetch_count = config.prefetch_count;
        let queue_parallelism = config.queue_parallelism;

        let handle = tokio::spawn(async move {
            let pb = mp.add(ProgressBar::new(queue_info.messages_ready));
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            pb.set_message(queue_info.name.clone());

            let result = worker::drain_queue(
                queue_info,
                source_conn,
                dest_conn,
                pb.clone(),
                consume_timeout,
                prefetch_count,
                queue_parallelism,
            )
            .await;

            if let Err(e) = &result {
                pb.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());
                pb.finish_with_message(format!(
                    "Error processing queue {}: {}",
                    pb.message(),
                    e
                ));
            } else {
                pb.finish_with_message(format!("Finished {}", pb.message()));
            }

            drop(permit); // Release the semaphore permit
            result
        });
        worker_handles.push(handle);
    }

    let mut total_errors = 0;
    for handle in worker_handles {
        match handle.await? {
            Ok(_) => {}
            Err(e) => {
                error!("A worker failed: {}", e);
                total_errors += 1;
            }
        }
    }
    multi_progress.clear()?;

    // endregion: --- Phase 3: Message Migration ---

    // region: --- Phase 4: Completion & Cleanup ---

    let duration = start_time.elapsed();
    info!("All workers have completed.");

    source_conn.close(200, "work complete").await?;
    dest_conn.close(200, "work complete").await?;
    info!("Connections gracefully closed.");

    println!("\n--- Migration Complete ---");
    println!("Duration: {:.2?}", duration);
    println!("Total messages moved: {}", total_messages);
    if total_errors > 0 {
        warn!("Completed with {} errors. Please check the logs.", total_errors);
    }
    println!("--------------------------");

    info!("Application shutting down.");

    // endregion: --- Phase 4: Completion & Cleanup ---

    Ok(())
}

fn setup_logging(config: &Config) -> Result<()> {
    let mut dispatch = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}] {}",
                record.level(),
                record.target(),
                message
            ))
        })
        .level(config.log_level)
        .chain(std::io::stdout());

    if let Some(log_file) = &config.log_file_path {
        dispatch = dispatch.chain(fern::log_file(log_file)?);
    }

    dispatch.apply()?;
    Ok(())
}
