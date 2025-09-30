use async_trait::async_trait;
use reqwest::Client;

use crate::request_id::X_REQUEST_ID;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait AlphaClient: Send + Sync {
    async fn send(
        &self,
        url: String,
        request_id: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Clone)]
pub(crate) struct SimpleAlphaClient {
    client: Client,
}

impl SimpleAlphaClient {
    pub(crate) fn new(client: Client) -> Self {
        SimpleAlphaClient { client }
    }
}

#[async_trait]
impl AlphaClient for SimpleAlphaClient {
    async fn send(
        &self,
        url: String,
        request_id: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self
            .client
            .get(&url)
            .header(X_REQUEST_ID, request_id)
            .send()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        let body = resp
            .text()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        Ok(body)
    }
}
