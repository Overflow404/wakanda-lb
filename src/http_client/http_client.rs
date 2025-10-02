use async_trait::async_trait;

use crate::http_client::{error::Error, request::Request, response::Response};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn execute(&self, request: Request) -> Result<Response, Error>;
}
