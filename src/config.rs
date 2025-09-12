use clap::Parser;
use log::LevelFilter;

/// A console application to migrate messages from a source RabbitMQ cluster to a destination cluster.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// Hostname of the source RabbitMQ cluster.
    #[arg(long, env = "SOURCE_HOST", default_value = "localhost")]
    pub source_host: String,
    /// Port of the source RabbitMQ cluster.
    #[arg(long, env = "SOURCE_PORT", default_value_t = 5672)]
    pub source_port: u16,
    /// Username for the source RabbitMQ cluster.
    #[arg(long, env = "SOURCE_USERNAME", default_value = "guest")]
    pub source_username: String,
    /// Password for the source RabbitMQ cluster.
    #[arg(long, env = "SOURCE_PASSWORD", default_value = "guest")]
    pub source_password: String,

    /// Hostname of the destination RabbitMQ cluster.
    #[arg(long, env = "DEST_HOST", default_value = "localhost")]
    pub dest_host: String,
    /// Port of the destination RabbitMQ cluster.
    #[arg(long, env = "DEST_PORT", default_value_t = 5672)]
    pub dest_port: u16,
    /// Username for the destination RabbitMQ cluster.
    #[arg(long, env = "DEST_USERNAME", default_value = "guest")]
    pub dest_username: String,
    /// Password for the destination RabbitMQ cluster.
    #[arg(long, env = "DEST_PASSWORD", default_value = "guest")]
    pub dest_password: String,

    /// Optional regex to filter queues by name.
    #[arg(long, env = "QUEUE_REGEX_FILTER")]
    pub queue_regex_filter: Option<String>,

    /// Maximum number of concurrent workers.
    #[arg(long, env = "MAX_PARALLEL_WORKERS", default_value_t = 1)]
    pub max_parallel_workers: usize,

    /// Path to the log file. If not provided, logs are only written to the console.
    #[arg(long, env = "LOG_FILE_PATH")]
    pub log_file_path: Option<String>,

    /// Minimum log level to output (e.g., INFO, WARN, ERROR).
    #[arg(long, env = "LOG_LEVEL", default_value_t = LevelFilter::Info)]
    pub log_level: LevelFilter,

    /// RabbitMQ Management API port for the source cluster.
    #[arg(long, env = "SOURCE_MGMT_PORT", default_value_t = 15672)]
    pub source_mgmt_port: u16,
}
