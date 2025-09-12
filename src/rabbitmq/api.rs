use crate::config::Config;
use crate::error::AppError;
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct QueueInfo {
    pub name: String,
    pub messages: u64,
    pub messages_ready: u64,
}

pub struct ManagementApiClient {
    client: Client,
    api_url: String,
    username: String,
    password: String,
}

impl ManagementApiClient {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            api_url: format!("http://{}:{}", config.source_host, config.source_mgmt_port),
            username: config.source_username.clone(),
            password: config.source_password.clone(),
        }
    }

    /// Fetches all queues from the source RabbitMQ cluster.
    pub async fn get_queues(&self) -> Result<Vec<QueueInfo>, AppError> {
        let url = format!("{}/api/queues", self.api_url);
        let queues: Vec<QueueInfo> = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?
            .json()
            .await?;
        Ok(queues)
    }
}
