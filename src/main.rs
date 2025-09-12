mod config;
mod error;
mod rabbitmq;
mod worker;

use crate::config::Config;
use anyhow::Result;
use clap::Parser;
use log::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // --- Phase 1: Initialization & Validation ---
    let config = Config::parse();
    setup_logging(&config)?;
    info!("Initialization complete. Configuration loaded and validated.");
    info!("Source: {}:{}", config.source_host, config.source_port);
    info!("Destination: {}:{}", config.dest_host, config.dest_port);

    // TODO: Validate connections to both RabbitMQ clusters.
    // If either fails, log an ERROR and exit.

    // --- Phase 2: Pre-Flight Check & Discovery ---
    info!("Starting pre-flight check...");
    // TODO:
    // 1. Create Management API Client.
    // 2. Query source for all queues with messages.
    // 3. Filter queues with `queue_regex_filter`.
    // 4. Verify corresponding destination queues exist. Exit on error.
    // 5. Present final list and total message count to the user.
    // 6. Prompt for confirmation.
    println!("Discovered X queues with a total of Y messages to migrate.");
    println!("Press Enter to begin migration or Ctrl+C to abort...");
    std::io::stdin().read_line(&mut String::new())?;

    // --- Phase 3: Message Migration ---
    info!("Starting message migration...");
    // TODO:
    // 1. Create a Queue Coordinator.
    // 2. Use a Tokio Semaphore to limit concurrency to `max_parallel_workers`.
    // 3. For each queue, spawn a `worker::drain_queue` task.
    // 4. Use `indicatif` for progress reporting.
    // 5. Wait for all workers to complete.

    // --- Phase 4: Completion & Cleanup ---
    info!("All queues have been drained.");
    // TODO:
    // 1. Gracefully shut down connections.
    // 2. Print a final summary report (total messages, duration, etc.).
    info!("Migration complete. Application shutting down.");

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
