use crate::select_server_service::{
    select_server_service::SelectServerService,
    select_server_service_error::SelectServerServiceError,
    select_server_service_request::SelectServerServiceRequest,
    select_server_service_response::SelectServerServiceResponse,
};

#[derive(Clone)]
pub struct RoundRobinSelectServerService {
    pub target_servers: Vec<String>,
}

impl RoundRobinSelectServerService {
    pub fn new(target_servers: Vec<String>) -> RoundRobinSelectServerService {
        Self { target_servers }
    }
}

impl SelectServerService for RoundRobinSelectServerService {
    fn execute(
        &self,
        _request: SelectServerServiceRequest,
    ) -> Result<SelectServerServiceResponse, SelectServerServiceError> {
        todo!()
    }
}
