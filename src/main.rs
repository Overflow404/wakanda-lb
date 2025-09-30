mod cli_arguments;
mod http_client;
mod request_id;
mod route;

use crate::http_client::alpha_client::{AlphaClient, SimpleAlphaClient};
use crate::request_id::{AlphaRequestId, UNKNOWN_REQUEST_ID, X_REQUEST_ID};
use crate::route::health::health;
use crate::{cli_arguments::CliArguments, route::forward::forward};
use axum::extract::Request;
use axum::{Router, routing::get};
use clap::Parser;
use reqwest::Client;
use std::sync::Arc;
use tower_http::request_id::{PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Clone)]
pub(crate) struct ServerState {
    client: Arc<dyn AlphaClient + Send + Sync>,
    target_servers_base_url: String,
}

pub(crate) fn router(server_state: ServerState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/", get(forward))
        .with_state(server_state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let request_id = request
                        .headers()
                        .get(X_REQUEST_ID)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or(UNKNOWN_REQUEST_ID);

                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                        request_id = %request_id,
                    )
                })
                .on_response(DefaultOnResponse::new().include_headers(true)),
        )
        .layer(PropagateRequestIdLayer::new(X_REQUEST_ID))
        .layer(SetRequestIdLayer::new(
            X_REQUEST_ID.clone(),
            AlphaRequestId::default(),
        ))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: CliArguments = CliArguments::parse();

    let tcp_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port))
        .await
        .unwrap();

    info!("Server started on port {}", args.port);

    let alpha_client = Arc::new(SimpleAlphaClient::new(Client::new()));
    let target_servers_base_url = String::from(args.target_servers_base_url);

    let state = ServerState {
        client: alpha_client,
        target_servers_base_url,
    };

    axum::serve(tcp_listener, router(state)).await.unwrap();
}

#[cfg(test)]
mod tests {
    use crate::http_client::alpha_client::MockAlphaClient;
    use crate::{ServerState, router};
    use axum::body::{Body, Bytes};
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;
    use uuid::Uuid;

    fn build_router_with_mock(url: String, response: &str) -> axum::Router {
        let mut mock = MockAlphaClient::default();
        let response_owned = response.to_string();
        let url_for_mock = url.clone();

        mock.expect_send()
            .withf(move |req_url, _| *req_url == url_for_mock)
            .returning(move |_, _| {
                let resp_clone = response_owned.clone();
                Ok(resp_clone)
            });

        router(ServerState {
            client: Arc::new(mock),
            target_servers_base_url: url,
        })
    }

    #[tokio::test]
    async fn should_expose_the_health_check_endpoint() {
        let router = build_router_with_mock(String::from("http://localhost:3000/health"), "");

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body();
        let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        assert_eq!(body_bytes, Bytes::from_static(b"PONG"));
    }

    #[tokio::test]
    async fn should_expose_the_forward_endpoint() {
        let router = build_router_with_mock(String::from("http://localhost:3000/"), "OK");

        let mut request = Request::builder().uri("/").body(Body::empty()).unwrap();

        request.extensions_mut().insert(Uuid::new_v4());

        let response = router.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn should_enrich_response_headers_with_request_id() {
        let router = build_router_with_mock(String::from("http://localhost:3000/health"), "");

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let headers = response.headers();

        assert!(headers.get("x-request-id").is_some());
    }
}
