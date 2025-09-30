use async_trait::async_trait;

use crate::forward_service::forward_service_request::ForwardServiceRequest;
use crate::forward_service::forward_service_response::ForwardServiceError;
use crate::forward_service::forward_service_response::ForwardServiceResponse;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ForwardService: Send + Sync {
    async fn execute(
        &self,
        target_url: &str,
        request: ForwardServiceRequest,
    ) -> Result<ForwardServiceResponse, ForwardServiceError>;
}
