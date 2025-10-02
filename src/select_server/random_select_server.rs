use std::sync::{
    Arc, RwLock,
};

use rand::Rng;

use crate::select_server::{
    error::Error, select_server::SelectServer, request::Request, response::Response
};

pub struct RandomSelectServer {
    target_servers: Arc<RwLock<Vec<String>>>,
}

impl RandomSelectServer {
    pub fn new(target_servers: Arc<RwLock<Vec<String>>>) -> RandomSelectServer {
        Self { target_servers }
    }
}

impl SelectServer for RandomSelectServer {
    fn execute(
        &self,
        _request: Request,
    ) -> Result<Response, Error> {
        let target_servers = self
            .target_servers
            .read()
            .map_err(|_| Error::PoisonedRead)?;

        if target_servers.is_empty() {
            return Err(Error::NoOneIsAlive);
        }

        let len = target_servers.len();
        let random_index = rand::rng().random_range(0..len);

        Ok(Response {
            server: target_servers[random_index].clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use crate::select_server::{
        error::Error, random_select_server::RandomSelectServer, select_server::SelectServer, request::Request
    };

    #[test]
    fn should_return_an_error_if_empty_targets() {
        let random_select_server = RandomSelectServer::new(Arc::new(RwLock::new(Vec::new())));

        let error = random_select_server
            .execute(Request {})
            .err()
            .unwrap();

        assert_eq!(error, Error::NoOneIsAlive)
    }

    #[test]
    fn should_return_the_next_targets() {
        let server1 = String::from("server1");
        let server2 = String::from("server2");

        let random_select_server = RandomSelectServer::new(Arc::new(RwLock::new(Vec::from([
            server1.clone(),
            server2.clone(),
        ]))));

        let result = random_select_server.execute(Request {});
        let selected = result.unwrap().server;
        assert!(selected == server1 || selected == server2);

        let result = random_select_server.execute(Request {});
        let selected = result.unwrap().server;
        assert!(selected == server1 || selected == server2);

        let result = random_select_server.execute(Request {});
        let selected = result.unwrap().server;
        assert!(selected == server1 || selected == server2);
    }
}
