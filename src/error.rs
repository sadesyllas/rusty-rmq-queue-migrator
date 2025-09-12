use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("RabbitMQ connection error: {0}")]
    ConnectionError(#[from] lapin::Error),

    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
