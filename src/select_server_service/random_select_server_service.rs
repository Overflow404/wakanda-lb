use std::sync::atomic::{AtomicUsize, Ordering};

use rand::Rng;

use crate::select_server_service::{
    select_server_service::SelectServerService,
    select_server_service_error::SelectServerServiceError,
    select_server_service_request::SelectServerServiceRequest,
    select_server_service_response::SelectServerServiceResponse,
};

pub struct RandomSelectServerService {
    target_servers: Vec<String>,
    current_server_index: AtomicUsize,
}

impl RandomSelectServerService {
    pub fn new(target_servers: Vec<String>) -> RandomSelectServerService {
        Self {
            target_servers,
            current_server_index: AtomicUsize::new(0),
        }
    }
}

impl SelectServerService for RandomSelectServerService {
    fn execute(
        &self,
        _request: SelectServerServiceRequest,
    ) -> Result<SelectServerServiceResponse, SelectServerServiceError> {
        if self.target_servers.is_empty() {
            return Err(SelectServerServiceError::NoOneIsAlive);
        }

        let random_index = rand::rng().random_range(0..self.target_servers.len());
        self.current_server_index
            .store(random_index, Ordering::Relaxed);

        Ok(SelectServerServiceResponse {
            server: self.target_servers[random_index].clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::select_server_service::{
        random_select_server_service::RandomSelectServerService,
        select_server_service::SelectServerService,
        select_server_service_error::SelectServerServiceError,
        select_server_service_request::SelectServerServiceRequest,
    };

    #[test]
    fn should_return_an_error_if_empty_targets() {
        let service = RandomSelectServerService::new(Vec::new());

        let error = service
            .execute(SelectServerServiceRequest {})
            .err()
            .unwrap();

        assert_eq!(error, SelectServerServiceError::NoOneIsAlive)
    }

    #[test]
    fn should_return_the_next_targets() {
        let server1 = String::from("server1");
        let server2 = String::from("server2");

        let service = RandomSelectServerService::new(Vec::from([server1.clone(), server2.clone()]));

        let result = service.execute(SelectServerServiceRequest {});
        let selected = result.unwrap().server;
        assert!(selected == server1 || selected == server2);

        let result = service.execute(SelectServerServiceRequest {});
        let selected = result.unwrap().server;
        assert!(selected == server1 || selected == server2);

        let result = service.execute(SelectServerServiceRequest {});
        let selected = result.unwrap().server;
        assert!(selected == server1 || selected == server2);
    }
}
