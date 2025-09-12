use crate::config::Config;
use crate::error::AppError;
use log::{info, warn};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Deserialize, Clone)]
pub struct QueueInfo {
    pub name: String,
    pub messages_ready: u64,
}

#[derive(Deserialize)]
struct PaginatedQueueResponse {
    items: Vec<QueueInfo>,
}

#[derive(Deserialize)]
struct ManagementApiErrorResponse {
    error: String,
    // reason: String,
}

pub struct ManagementApiClient {
    client: Client,
    api_url: String,
    username: String,
    password: String,
    vhost: String,
}

impl ManagementApiClient {
    pub fn new(config: &Config) -> Self {
        let protocol = if config.source_mgmt_use_ssl { "https" } else { "http" };
        let api_url = format!(
            "{}://{}:{}",
            protocol, config.source_host, config.source_mgmt_port
        );
        Self {
            client: Client::new(),
            api_url,
            username: config.source_username.clone(),
            password: config.source_password.as_ref().unwrap().clone(),
            vhost: urlencoding::encode(&config.source_vhost).to_string(),
        }
    }

    /// Fetches all queues from the source RabbitMQ cluster using pagination.
    pub async fn get_queues(
        &self,
        regex_filter: Option<&str>,
        api_retries: u32,
        api_retry_delay_secs: u64,
    ) -> Result<Vec<QueueInfo>, AppError> {
        const PAGE_SIZE: usize = 500;
        let mut page = 1;
        let mut all_queues = Vec::new();

        info!(
            "Starting to fetch queues from management API with page size {}.",
            PAGE_SIZE
        );

        'pagination: loop {
            let mut url = format!(
                "{}/api/queues/{}?page={}&page_size={}",
                self.api_url, self.vhost, page, PAGE_SIZE
            );

            if let Some(filter) = regex_filter {
                url.push_str(&format!(
                    "&name={}&use_regex=true",
                    urlencoding::encode(filter)
                ));
            }

            for attempt in 0..=api_retries {
                let response_result = self
                    .client
                    .get(&url)
                    .basic_auth(&self.username, Some(&self.password))
                    .send()
                    .await;

                let response = match response_result {
                    Ok(res) => res,
                    Err(e) => {
                        if attempt == api_retries {
                            return Err(e.into());
                        }
                        warn!(
                            "API call failed on attempt {}/{}. Retrying in {}s... Error: {}",
                            attempt + 1,
                            api_retries,
                            api_retry_delay_secs,
                            e
                        );
                        sleep(Duration::from_secs(api_retry_delay_secs)).await;
                        continue;
                    }
                };

                let status = response.status();
                let text = response.text().await?;

                if status.is_success() {
                    let page_data: PaginatedQueueResponse = serde_json::from_str(&text)?;
                    let fetched_queues = page_data.items;

                    if fetched_queues.is_empty() {
                        info!(
                            "Finished fetching queues (received empty page). Total found: {}.",
                            all_queues.len()
                        );
                        break 'pagination;
                    }

                    all_queues.extend(fetched_queues);
                    info!(
                        "Fetched page {}. Total queues so far: {}.",
                        page,
                        all_queues.len()
                    );

                    page += 1;
                    sleep(Duration::from_secs(1)).await;
                    break; // Success, break retry loop and continue to next page
                }

                if status == StatusCode::BAD_REQUEST {
                    if let Ok(error_response) =
                        serde_json::from_str::<ManagementApiErrorResponse>(&text)
                    {
                        if error_response.error == "page_out_of_range" {
                            info!(
                                "Finished fetching queues (hit page_out_of_range). Total found: {}.",
                                all_queues.len()
                            );
                            break 'pagination;
                        }
                    }
                }

                if attempt == api_retries {
                    return Err(AppError::ManagementApiError(format!(
                        "Management API request failed after all retries with status {}: {}",
                        status, text
                    )));
                }

                warn!(
                    "API call failed with status {} on attempt {}/{}. Retrying in {}s...",
                    status,
                    attempt + 1,
                    api_retries,
                    api_retry_delay_secs
                );
                sleep(Duration::from_secs(api_retry_delay_secs)).await;
            }
        }

        Ok(all_queues)
    }
}
