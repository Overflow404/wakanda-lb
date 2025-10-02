use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use async_trait::async_trait;
use bytes::Bytes;
use tokio::time;
use tracing::{error, info, warn};

use crate::{
    background_health_checker::background_health_checker::BackgroundChecker,
    http_client::{
        http_client::HttpClient,
        request::{
            RequestMethod, Request,
            RequestHeaders,
        },
    },
};

pub struct TimedBackgroundChecker {
    pub http_client: Arc<dyn HttpClient>,
    pub target_servers: Arc<RwLock<Vec<String>>>,
    pub health_endpoint: String,
    pub polling_interval: Duration,
}

#[async_trait]
impl BackgroundChecker for TimedBackgroundChecker {
    async fn execute(&self) {
        info!(
            "Starting timed background checker with {:?} polling interval",
            self.polling_interval
        );

        let mut interval = time::interval(self.polling_interval);

        loop {
            interval.tick().await;

            let servers_to_check = {
                match self.target_servers.read() {
                    Ok(guard) => guard.clone(),
                    Err(error) => {
                        error!("Failed to read target servers: {}", error);
                        continue;
                    }
                }
            };

            if servers_to_check.is_empty() {
                warn!("No healthy servers before the timed background check!");
                continue;
            }

            info!("Checking health of {} servers", servers_to_check.len());

            let mut healthy_servers = Vec::new();

            for target_server in servers_to_check.iter() {
                let request = Request {
                    method: RequestMethod::Get,
                    url: format!("{}{}", target_server, self.health_endpoint),
                    headers: RequestHeaders::default(),
                    body: Bytes::new(),
                };

                match self.http_client.execute(request).await {
                    Ok(response) => {
                        if response.status == 200 {
                            healthy_servers.push(target_server.clone());
                            info!("Server {} is healthy", target_server);
                        } else {
                            warn!(
                                "Server {} returned unhealthy status: {}",
                                target_server, response.status
                            );
                        }
                    }
                    Err(error) => {
                        error!("Server {} failed health check: {}", target_server, error);
                    }
                }
            }

            match self.target_servers.write() {
                Ok(mut guard) => {
                    *guard = healthy_servers;
                    info!("Updated target servers: {:#?}", *guard);

                    if guard.is_empty() {
                        error!("No healthy servers after the timed background check!");
                    }
                }
                Err(error) => {
                    error!("Failed to update target servers: {}", error);
                }
            }
        }
    }
}
