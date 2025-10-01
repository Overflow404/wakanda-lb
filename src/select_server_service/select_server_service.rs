use crate::select_server_service::{
    select_server_service_error::SelectServerServiceError, select_server_service_request::SelectServerServiceRequest, select_server_service_response::SelectServerServiceResponse
};

#[cfg_attr(test, mockall::automock)]
pub trait SelectServerService: Send + Sync {
    fn execute(&self, request: SelectServerServiceRequest) -> Result<SelectServerServiceResponse, SelectServerServiceError>;
}
