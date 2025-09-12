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
    /// Password for the source RabbitMQ cluster. If not provided, you will be prompted to enter it.
    #[arg(long, env = "SOURCE_PASSWORD")]
    pub source_password: Option<String>,

    /// Hostname of the destination RabbitMQ cluster.
    #[arg(long, env = "DEST_HOST", default_value = "localhost")]
    pub dest_host: String,
    /// Port of the destination RabbitMQ cluster.
    #[arg(long, env = "DEST_PORT", default_value_t = 5672)]
    pub dest_port: u16,
    /// Username for the destination RabbitMQ cluster.
    #[arg(long, env = "DEST_USERNAME", default_value = "guest")]
    pub dest_username: String,
    /// Password for the destination RabbitMQ cluster. If not provided, you will be prompted to enter it.
    #[arg(long, env = "DEST_PASSWORD")]
    pub dest_password: Option<String>,

    /// Virtual host for the source RabbitMQ cluster.
    #[arg(long, env = "SOURCE_VHOST", default_value = "/")]
    pub source_vhost: String,

    /// Virtual host for the destination RabbitMQ cluster.
    #[arg(long, env = "DEST_VHOST", default_value = "/")]
    pub dest_vhost: String,

    /// Optional regex to filter queues by name.
    #[arg(long, env = "QUEUE_REGEX_FILTER")]
    pub queue_regex_filter: Option<String>,

    /// Maximum number of concurrent workers. If not provided, defaults to the number of logical CPUs.
    #[arg(long, env = "MAX_PARALLEL_WORKERS")]
    pub max_parallel_workers: Option<usize>,

    /// The prefetch count for the consumer on the source queue.
    #[arg(long, env = "PREFETCH_COUNT", default_value_t = 1000)]
    pub prefetch_count: u16,

    /// The number of concurrent workers to spawn for each queue.
    #[arg(long, env = "QUEUE_PARALLELISM", default_value_t = 1)]
    pub queue_parallelism: u16,

    /// Path to the log file. If not provided, logs are only written to the console.
    #[arg(long, env = "LOG_FILE_PATH")]
    pub log_file_path: Option<String>,

    /// Minimum log level to output (e.g., INFO, WARN, ERROR).
    #[arg(long, env = "LOG_LEVEL", default_value_t = LevelFilter::Info)]
    pub log_level: LevelFilter,

    /// RabbitMQ Management API port for the source cluster.
    #[arg(long, env = "SOURCE_MGMT_PORT", default_value_t = 15672)]
    pub source_mgmt_port: u16,

    /// Use HTTPS for the source RabbitMQ Management API.
    #[arg(long, env = "SOURCE_MGMT_USE_SSL")]
    pub source_mgmt_use_ssl: bool,

    /// Number of retries for a failing management API call.
    #[arg(long, env = "API_RETRIES", default_value_t = 3)]
    pub api_retries: u32,

    /// Delay in seconds between retries for a failing management API call.
    #[arg(long, env = "API_RETRY_DELAY_SECS", default_value_t = 5)]
    pub api_retry_delay_secs: u64,

    /// Timeout in seconds for a worker to wait for a message before assuming a queue is empty.
    #[arg(long, env = "CONSUME_TIMEOUT", default_value_t = 5)]
    pub consume_timeout: u64,

    /// Only run pre-flight checks (discovery and validation) and then exit.
    #[arg(long)]
    pub only_preflight: bool,
}
