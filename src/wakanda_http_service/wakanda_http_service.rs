use async_trait::async_trait;

use crate::wakanda_http_service::{
    wakanda_http_service_request::WakandaHttpServiceRequest,
    wakanda_http_service_response::{WakandaHttpServiceError, WakandaHttpServiceResponse},
};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WakandaHttpService: Send + Sync {
    async fn execute(
        &self,
        request: WakandaHttpServiceRequest,
    ) -> Result<WakandaHttpServiceResponse, WakandaHttpServiceError>;
}
