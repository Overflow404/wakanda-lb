use async_trait::async_trait;
use bytes::Bytes;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::time;
use tracing::{error, info, warn};

use crate::{
    background_health_checker::background_health_checker::BackgroundChecker,
    http_client::{
        http_client::HttpClient,
        request::{Request, RequestHeaders, RequestMethod},
    },
};

pub(crate) struct TimedBackgroundChecker {
    pub http_client: Arc<dyn HttpClient>,
    pub all_servers: Vec<String>,
    pub healthy_servers: Arc<RwLock<Vec<String>>>,
    pub health_endpoint: String,
    pub polling_interval: Duration,
}

impl TimedBackgroundChecker {
    pub fn new(
        http_client: Arc<dyn HttpClient>,
        servers: Vec<String>,
        health_endpoint: String,
        polling_interval: Duration,
    ) -> Self {
        let healthy_servers = Arc::new(RwLock::new(servers.clone()));
        Self {
            http_client,
            all_servers: servers,
            healthy_servers,
            health_endpoint,
            polling_interval,
        }
    }

    async fn is_server_healthy(&self, server: &str) -> bool {
        let request = Request {
            method: RequestMethod::Get,
            url: format!("{}{}", server, self.health_endpoint),
            headers: RequestHeaders::default(),
            body: Bytes::new(),
        };

        match tokio::time::timeout(Duration::from_secs(5), self.http_client.execute(request)).await
        {
            Ok(Ok(response)) => {
                if response.status == 200 {
                    true
                } else {
                    warn!(
                        "Server {} returned unhealthy status: {}",
                        server, response.status
                    );
                    false
                }
            }
            Ok(Err(error)) => {
                warn!("Server {} failed health check: {}", server, error);
                false
            }
            Err(_) => {
                warn!("Server {} health check timed out", server);
                false
            }
        }
    }
}

#[async_trait]
impl BackgroundChecker for TimedBackgroundChecker {
    async fn execute(&self) {
        info!(
            "Starting timed background checker with {:?} polling interval",
            self.polling_interval
        );
        info!(
            "Monitoring {} servers: {:?}",
            self.all_servers.len(),
            self.all_servers
        );

        let mut interval = time::interval(self.polling_interval);

        loop {
            interval.tick().await;

            if self.all_servers.is_empty() {
                warn!("No servers configured to check");
                continue;
            }

            info!("Checking health of {} servers", self.all_servers.len());

            let mut new_healthy_servers = Vec::new();

            for server in self.all_servers.iter() {
                if self.is_server_healthy(server).await {
                    new_healthy_servers.push(server.clone());
                    info!("✓ Server {} is healthy", server);
                } else {
                    info!("✖ Server {} is unhealthy", server);
                }
            }

            match self.healthy_servers.write() {
                Ok(mut guard) => {
                    let previously_healthy = guard.len();
                    *guard = new_healthy_servers;
                    let currently_healthy = guard.len();

                    if currently_healthy != previously_healthy {
                        info!(
                            "Health status changed: {} → {} healthy servers",
                            previously_healthy, currently_healthy
                        );
                    }

                    info!("Current healthy servers: {:#?}", *guard);

                    if guard.is_empty() {
                        error!("No healthy servers available!");
                    }
                }
                Err(error) => {
                    error!("Failed to update healthy servers list: {}", error);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use bytes::Bytes;

    use crate::background_health_checker::timed_background_health_checker::TimedBackgroundChecker;
    use crate::http_client::error::Error;
    use crate::http_client::http_client::{HttpClient, MockHttpClient};
    use crate::http_client::request::RequestHeaders;
    use crate::http_client::response::Response;

    fn make_timed_background_checker(
        http_client: Arc<dyn HttpClient>,
        servers: Vec<String>,
    ) -> TimedBackgroundChecker {
        TimedBackgroundChecker::new(
            http_client,
            servers,
            "/health".to_string(),
            Duration::from_millis(100),
        )
    }

    #[tokio::test]
    async fn all_servers_are_healthy() {
        let mut mock = MockHttpClient::new();
        mock.expect_execute().returning(|_| {
            Ok(Response {
                status: 200,
                headers: RequestHeaders::default(),
                body: Bytes::new(),
            })
        });

        let servers = vec!["http://server1".to_string(), "http://server2".to_string()];
        let checker = make_timed_background_checker(Arc::new(mock), servers.clone());

        for server in &checker.all_servers {
            assert!(checker.is_server_healthy(server).await);
        }

        let healthy = checker.healthy_servers.read().unwrap();
        assert_eq!(healthy.len(), servers.len());
    }

    #[tokio::test]
    async fn one_server_is_unhealthy() {
        let mut mock = MockHttpClient::new();

        mock.expect_execute()
            .withf(|req| req.url.contains("server1"))
            .returning(|_| {
                Ok(Response {
                    status: 200,
                    headers: RequestHeaders::default(),
                    body: Bytes::new(),
                })
            });

        mock.expect_execute()
            .withf(|req| req.url.contains("server2"))
            .returning(|_| {
                Ok(Response {
                    status: 503,
                    headers: RequestHeaders::default(),
                    body: Bytes::new(),
                })
            });

        let servers = vec!["http://server1".to_string(), "http://server2".to_string()];
        let checker = make_timed_background_checker(Arc::new(mock), servers);

        assert!(checker.is_server_healthy("http://server1").await);
        assert!(!checker.is_server_healthy("http://server2").await);
    }

    #[tokio::test]
    async fn recovered_server_should_resume_receiving_traffic() {
        let mut mock = MockHttpClient::new();

        mock.expect_execute().times(1).return_once(|_| {
            Ok(Response {
                status: 503,
                headers: RequestHeaders::default(),
                body: Bytes::new(),
            })
        });

        mock.expect_execute().times(1).return_once(|_| {
            Ok(Response {
                status: 200,
                headers: RequestHeaders::default(),
                body: Bytes::new(),
            })
        });

        let servers = vec!["http://server1".to_string()];
        let checker = make_timed_background_checker(Arc::new(mock), servers);

        assert!(!checker.is_server_healthy("http://server1").await);

        assert!(checker.is_server_healthy("http://server1").await);
    }

    #[tokio::test]
    async fn should_mark_server_as_unhealthy_when_network_error() {
        let mut mock = MockHttpClient::new();
        mock.expect_execute()
            .returning(|_| Err(Error::Network("Connection refused".to_string())));

        let servers = vec!["http://server1".to_string()];
        let checker = make_timed_background_checker(Arc::new(mock), servers);

        assert!(!checker.is_server_healthy("http://server1").await);
    }

    #[tokio::test]
    async fn all_servers_unhealthy() {
        let mut mock = MockHttpClient::new();
        mock.expect_execute().returning(|_| {
            Ok(Response {
                status: 500,
                headers: RequestHeaders::default(),
                body: Bytes::new(),
            })
        });

        let servers = vec!["http://server1".to_string(), "http://server2".to_string()];
        let checker = make_timed_background_checker(Arc::new(mock), servers.clone());

        for server in &checker.all_servers {
            assert!(!checker.is_server_healthy(server).await);
        }

        let mut healthy = checker.healthy_servers.write().unwrap();
        healthy.clear();
        assert_eq!(healthy.len(), 0);
    }

    #[tokio::test]
    async fn empty_server_list() {
        let mock = MockHttpClient::new();
        let checker = make_timed_background_checker(Arc::new(mock), vec![]);

        assert_eq!(checker.all_servers.len(), 0);
        let healthy = checker.healthy_servers.read().unwrap();
        assert_eq!(healthy.len(), 0);
    }

    #[tokio::test]
    async fn should_call_the_correct_health_endpoint() {
        let mut mock = MockHttpClient::new();

        mock.expect_execute()
            .withf(|req| req.url == "http://server1/health")
            .times(1)
            .returning(|_| {
                Ok(Response {
                    status: 200,
                    headers: RequestHeaders::default(),
                    body: Bytes::new(),
                })
            });

        let servers = vec!["http://server1".to_string()];
        let checker = make_timed_background_checker(Arc::new(mock), servers);

        checker.is_server_healthy("http://server1").await;
    }
}
