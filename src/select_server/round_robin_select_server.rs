use std::sync::{
    Arc, RwLock,
    atomic::{AtomicUsize, Ordering},
};

use crate::select_server::{
    error::Error, request::Request, response::Response, select_server::SelectServer,
};

pub(crate) struct RoundRobinSelectServer {
    target_servers: Arc<RwLock<Vec<String>>>,
    current_server_index: AtomicUsize,
}

impl RoundRobinSelectServer {
    pub(crate) fn new(target_servers: Arc<RwLock<Vec<String>>>) -> RoundRobinSelectServer {
        Self {
            target_servers,
            current_server_index: AtomicUsize::new(0),
        }
    }
}

impl SelectServer for RoundRobinSelectServer {
    fn execute(&self, _request: Request) -> Result<Response, Error> {
        let target_servers = match self.target_servers.read() {
            Ok(servers) => servers,
            Err(_) => return Err(Error::PoisonedRead),
        };

        if target_servers.is_empty() {
            return Err(Error::NoOneIsAlive);
        }

        let len = target_servers.len();

        let index = self
            .current_server_index
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.wrapping_add(1))
            })
            .unwrap();

        let index = index % len;
        Ok(Response {
            server: target_servers[index].clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use crate::select_server::{
        error::Error, request::Request, round_robin_select_server::RoundRobinSelectServer,
        select_server::SelectServer,
    };

    #[test]
    fn should_return_an_error_if_empty_targets() {
        let round_robin_select_server =
            RoundRobinSelectServer::new(Arc::new(RwLock::new(Vec::new())));

        let error = round_robin_select_server.execute(Request {}).err().unwrap();

        assert_eq!(error, Error::NoOneIsAlive)
    }

    #[test]
    fn should_return_the_next_targets() {
        let server1 = String::from("server1");
        let server2 = String::from("server2");

        let round_robin_select_server =
            RoundRobinSelectServer::new(Arc::new(RwLock::new(Vec::from([
                server1.clone(),
                server2.clone(),
            ]))));

        let mut result = round_robin_select_server
            .execute(Request {})
            .unwrap()
            .server;

        assert_eq!(result, server1);

        result = round_robin_select_server
            .execute(Request {})
            .unwrap()
            .server;

        assert_eq!(result, server2);

        result = round_robin_select_server
            .execute(Request {})
            .unwrap()
            .server;

        assert_eq!(result, server1);

        result = round_robin_select_server
            .execute(Request {})
            .unwrap()
            .server;

        assert_eq!(result, server2);
    }
}
