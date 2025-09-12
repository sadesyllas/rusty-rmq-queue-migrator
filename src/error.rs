use thiserror::Error;
use tokio::task::JoinError;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("RabbitMQ connection error: {0}")]
    ConnectionError(#[from] lapin::Error),

    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Management API request failed: {0}")]
    ManagementApiError(String),

    #[error("JSON deserialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Worker task failed to execute: {0}")]
    WorkerJoinError(#[from] JoinError),
}
