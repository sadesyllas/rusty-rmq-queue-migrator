use crate::error::AppError;
use lapin::{Channel, options::*, types::FieldTable};
use log::info;

/// Drains a single queue by consuming messages and republishing them to the destination.
pub async fn drain_queue(
    queue_name: String,
    source_channel: Channel,
    dest_channel: Channel,
) -> Result<(), AppError> {
    info!("[{}] Starting to drain queue.", queue_name);

    // TODO:
    // 1. Create a consumer on the source channel for `queue_name`.
    // 2. In a loop, for each message received:
    //    a. Publish the message to the destination channel to a queue with the same name.
    //       - Ensure all properties and headers are preserved.
    //    b. Wait for publisher confirm from the destination.
    //    c. If publish is successful, ACK the message on the source channel.
    //    d. If publish fails, NACK the message and requeue it.
    // 3. The loop terminates when the consumer is cancelled (queue is empty).
    // 4. Update progress using `indicatif`.

    info!("[{}] Finished draining queue.", queue_name);
    Ok(())
}
