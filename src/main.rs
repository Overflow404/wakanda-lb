pub mod background_health_checker;
pub(crate) mod cli_arguments;
pub(crate) mod http_client;
pub mod request_id;
pub mod select_server;

use crate::cli_arguments::{CliArguments, RoutingPolicy};
use clap::Parser;
use load_balancer::background_health_checker::background_health_checker::BackgroundChecker;
use load_balancer::{
    RandomSelectServer, ReqwestHttpClient, RoundRobinSelectServer, SelectServer, ServerState,
    TimedBackgroundChecker, router,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn setup_tracing_subscriber() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn make_background_checker(args: &CliArguments) -> Arc<TimedBackgroundChecker> {
    Arc::new(TimedBackgroundChecker::new(
        Arc::new(ReqwestHttpClient::default()),
        args.target_servers.clone(),
        args.target_servers_health_path.clone(),
        Duration::from_secs(args.health_checker_polling_seconds),
    ))
}

fn make_select_server(
    routing_policy: &RoutingPolicy,
    background_health_checker: &TimedBackgroundChecker,
) -> Arc<dyn SelectServer + Send + Sync> {
    match routing_policy {
        RoutingPolicy::RoundRobin => Arc::new(RoundRobinSelectServer::new(
            background_health_checker.get_healthy_servers(),
        )),
        RoutingPolicy::Random => Arc::new(RandomSelectServer::new(
            background_health_checker.get_healthy_servers(),
        )),
    }
}

fn make_server_state(select_server: Arc<dyn SelectServer + Send + Sync>) -> ServerState {
    let http_client = Arc::new(ReqwestHttpClient::default());
    ServerState {
        http_client,
        select_server,
    }
}

fn spawn_background_health_checker(background_health_checker: Arc<TimedBackgroundChecker>) {
    tokio::spawn(async move {
        background_health_checker.execute().await;
    });
}

async fn start_server(port: u16, state: ServerState) {
    let tcp_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("Failed to bind TCP listener");

    axum::serve(tcp_listener, router(state))
        .await
        .expect("Server failed to run");

    info!("Server started on port {}", port);
}

#[tokio::main]
async fn main() {
    setup_tracing_subscriber();

    let args = CliArguments::parse();

    let background_checker = make_background_checker(&args);
    let select_server = make_select_server(&args.routing_policy, &background_checker);
    let state = make_server_state(select_server);

    spawn_background_health_checker(background_checker);

    start_server(args.port, state).await;
}
