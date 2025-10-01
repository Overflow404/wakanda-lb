use std::sync::{
    Arc, RwLock,
    atomic::{AtomicUsize, Ordering},
};

use crate::select_server_service::{
    select_server_service::SelectServerService,
    select_server_service_error::SelectServerServiceError,
    select_server_service_request::SelectServerServiceRequest,
    select_server_service_response::SelectServerServiceResponse,
};

pub struct RoundRobinSelectServerService {
    target_servers: Arc<RwLock<Vec<String>>>,
    current_server_index: AtomicUsize,
}

impl RoundRobinSelectServerService {
    pub fn new(target_servers: Arc<RwLock<Vec<String>>>) -> RoundRobinSelectServerService {
        Self {
            target_servers,
            current_server_index: AtomicUsize::new(0),
        }
    }
}

impl SelectServerService for RoundRobinSelectServerService {
    fn execute(
        &self,
        _request: SelectServerServiceRequest,
    ) -> Result<SelectServerServiceResponse, SelectServerServiceError> {
        let target_servers = match self.target_servers.read() {
            Ok(servers) => servers,
            Err(_) => return Err(SelectServerServiceError::PoisonedRead),
        };

        if target_servers.is_empty() {
            return Err(SelectServerServiceError::NoOneIsAlive);
        }

        let len = target_servers.len();

        let index = self
            .current_server_index
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.wrapping_add(1))
            })
            .unwrap();

        let index = index % len;
        Ok(SelectServerServiceResponse {
            server: target_servers[index].clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use crate::select_server_service::{
        round_robin_select_server_service::RoundRobinSelectServerService,
        select_server_service::SelectServerService,
        select_server_service_error::SelectServerServiceError,
        select_server_service_request::SelectServerServiceRequest,
    };

    #[test]
    fn should_return_an_error_if_empty_targets() {
        let service = RoundRobinSelectServerService::new(Arc::new(RwLock::new(Vec::new())));

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

        let service = RoundRobinSelectServerService::new(Arc::new(RwLock::new(Vec::from([
            server1.clone(),
            server2.clone(),
        ]))));

        let mut result = service
            .execute(SelectServerServiceRequest {})
            .unwrap()
            .server;

        assert_eq!(result, server1);

        result = service
            .execute(SelectServerServiceRequest {})
            .unwrap()
            .server;

        assert_eq!(result, server2);

        result = service
            .execute(SelectServerServiceRequest {})
            .unwrap()
            .server;

        assert_eq!(result, server1);

        result = service
            .execute(SelectServerServiceRequest {})
            .unwrap()
            .server;

        assert_eq!(result, server2);
    }
}
