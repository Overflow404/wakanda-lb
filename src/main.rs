pub(crate) mod background_health_checker;
pub(crate) mod cli_arguments;
pub(crate) mod http_client;
pub(crate) mod request_id;
pub(crate) mod select_server;

use crate::background_health_checker::background_health_checker::BackgroundChecker;
use crate::background_health_checker::timed_background_health_checker::TimedBackgroundChecker;
use crate::cli_arguments::{CliArguments, RoutingPolicy};

use crate::http_client::error::Error as HttpClientError;
use crate::http_client::http_client::HttpClient;
use crate::http_client::request::{Request as HttpClientRequest, RequestMethod};
use crate::http_client::reqwest_http_client::ReqwestHttpClient;
use crate::http_client::response::Response as HttpClientResponse;
use crate::request_id::{LoadBalancerRequestId, UNKNOWN_REQUEST_ID, X_REQUEST_ID};
use crate::select_server::random_select_server::RandomSelectServer;
use crate::select_server::request::Request as SelectServerRequest;
use crate::select_server::round_robin_select_server::RoundRobinSelectServer;
use crate::select_server::select_server::SelectServer;

use axum::body::{Body, to_bytes};
use axum::extract::Request as AxumRequest;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::{Router, routing::get};
use clap::Parser;
use http::StatusCode;
use std::sync::Arc;
use std::time::Duration;
use tower_http::request_id::{PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::{error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Clone)]
pub(crate) struct ServerState {
    http_client: Arc<dyn HttpClient + Send + Sync>,
    select_server: Arc<dyn SelectServer>,
}

async fn health_endpoint() -> impl IntoResponse {
    info!("Health check executed");
    "PONG"
}

async fn proxy_endpoint(
    State(state): State<ServerState>,
    request: AxumRequest<Body>,
) -> impl IntoResponse {
    let (parts, body) = request.into_parts();

    let server = match state.select_server.execute(SelectServerRequest {}) {
        Ok(selected_server) => selected_server.server,
        Err(error) => {
            error!("No one is alive: {}", error);
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };

    let url = format!("{}{}", server, parts.uri.path().to_string());

    let headers = parts.headers.into();

    let body = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(error) => {
            error!("Failed body conversion into bytes: {}", error);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let method: RequestMethod = match (&parts.method).try_into() {
        Ok(method) => method,
        Err(error) => {
            error!("{}", error.to_string());
            return error.into_response();
        }
    };

    let result = state
        .http_client
        .execute(HttpClientRequest {
            method,
            headers,
            body,
            url,
        })
        .await;

    match result {
        Ok(http_client_response) => http_client_response.into(),
        Err(error) => {
            let (status, error) = error.into();
            error!("Error: {} Status: {}", error, status);

            (status, error).into_response()
        }
    }
}

impl From<HttpClientResponse> for Response<Body> {
    fn from(value: HttpClientResponse) -> Self {
        let mut response = Response::builder()
            .status(StatusCode::from_u16(value.status).unwrap_or(StatusCode::OK));

        for (k, v) in value.headers.iter() {
            response = response.header(k, v);
        }

        response.body(Body::from(value.body)).unwrap()
    }
}

impl From<HttpClientError> for (StatusCode, &str) {
    fn from(value: HttpClientError) -> Self {
        match value {
            HttpClientError::Network(_) => (StatusCode::BAD_GATEWAY, "Network error"),
            HttpClientError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, "Invalid request"),
            HttpClientError::Timeout => (StatusCode::GATEWAY_TIMEOUT, "Timeout"),
        }
    }
}

pub(crate) fn router(server_state: ServerState) -> Router {
    Router::new()
        .route("/health", get(health_endpoint))
        .route("/{*path}", any(proxy_endpoint))
        .route("/", any(proxy_endpoint))
        .with_state(server_state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &AxumRequest<_>| {
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
            X_REQUEST_ID,
            LoadBalancerRequestId::default(),
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

    let http_client = Arc::new(ReqwestHttpClient::default());
    let target_servers = args.target_servers;

    let background_checker = Arc::new(TimedBackgroundChecker::new(
        Arc::new(ReqwestHttpClient::default()),
        target_servers,
        args.target_servers_health_path,
        Duration::from_secs(args.health_checker_polling_seconds),
    ));

    let select_server: Arc<dyn SelectServer + Send + Sync> = match args.routing_policy {
        RoutingPolicy::RoundRobin => Arc::new(RoundRobinSelectServer::new(Arc::clone(
            &background_checker.healthy_servers,
        ))),
        RoutingPolicy::Random => Arc::new(RandomSelectServer::new(Arc::clone(
            &background_checker.healthy_servers,
        ))),
    };

    let state = ServerState {
        http_client,
        select_server,
    };

    tokio::spawn(async move {
        background_checker.execute().await;
    });

    axum::serve(tcp_listener, router(state)).await.unwrap();
}

#[cfg(test)]
mod tests {

    use crate::http_client::error::Error as HttpClientError;
    use crate::http_client::http_client::MockHttpClient;
    use crate::http_client::request::{RequestHeaders, RequestMethod};
    use crate::http_client::response::Response as HttpClientResponse;
    use crate::select_server::error::Error as SelectServerError;
    use crate::select_server::response::Response as SelectServerResponse;
    use crate::select_server::select_server::MockSelectServer;
    use crate::{ServerState, X_REQUEST_ID, router};
    use axum::body::{Body, Bytes};
    use axum::http::{Method, Request, StatusCode};
    use axum::response::Response as AxumResponse;
    use http::{HeaderMap, HeaderValue};
    use mockall::predicate::*;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn target_servers() -> Vec<String> {
        vec![String::from("http://target.com")]
    }

    fn build_router_with_mocks(
        target_servers: Vec<String>,
        setup_http_client_mock: impl FnOnce(&mut MockHttpClient),
        setup_select_server_mock: impl FnOnce(&mut MockSelectServer, Vec<String>),
    ) -> axum::Router {
        let mut http_client_mock = MockHttpClient::default();
        setup_http_client_mock(&mut http_client_mock);

        let mut select_server_mock = MockSelectServer::default();
        setup_select_server_mock(&mut select_server_mock, target_servers);

        router(ServerState {
            http_client: Arc::new(http_client_mock),
            select_server: Arc::new(select_server_mock),
        })
    }

    fn build_success_http_client_mock() -> impl FnOnce(&mut MockHttpClient) {
        |mock: &mut MockHttpClient| {
            mock.expect_execute().returning(|_| {
                Ok(HttpClientResponse {
                    status: 200,
                    headers: RequestHeaders::default(),
                    body: Bytes::from("OK"),
                })
            });
        }
    }
    fn first_one_select_server_mock() -> impl FnOnce(&mut MockSelectServer, Vec<String>) {
        |select_server_mock, target_servers| {
            let first_server = target_servers.clone().get(0).unwrap().clone();

            select_server_mock.expect_execute().returning(move |_| {
                Ok(SelectServerResponse {
                    server: first_server.clone(),
                })
            });
        }
    }

    #[tokio::test]
    async fn health_endpoint_returns_pong() {
        let router = build_router_with_mocks(
            target_servers(),
            build_success_http_client_mock(),
            first_one_select_server_mock(),
        );

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
        assert!(response.headers().get(X_REQUEST_ID).is_some());

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();

        assert_eq!(body_bytes, Bytes::from_static(b"PONG"));
    }

    #[tokio::test]
    async fn proxy_endpoint_sends_get_request() {
        let router = build_router_with_mocks(
            target_servers(),
            |http_client_mock| {
                http_client_mock
                    .expect_execute()
                    .withf(move |req| {
                        req.method == RequestMethod::Get
                            && req.url == "http://target.com/"
                            && req.body == Bytes::new()
                    })
                    .times(1)
                    .returning(|_| {
                        Ok(HttpClientResponse {
                            status: 200,
                            headers: RequestHeaders::default(),
                            body: Bytes::from("Success"),
                        })
                    });
            },
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body_bytes, Bytes::from("Success"));
    }

    #[tokio::test]
    async fn proxy_endpoint_preserves_response_status() {
        let router = build_router_with_mocks(
            target_servers(),
            |mock| {
                mock.expect_execute()
                    .withf(move |req| {
                        req.method == RequestMethod::Get
                            && req.url == "http://target.com/"
                            && req.body == Bytes::new()
                    })
                    .returning(|_| {
                        Ok(HttpClientResponse {
                            status: 201,
                            headers: RequestHeaders::default(),
                            body: Bytes::from("Created"),
                        })
                    });
            },
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn proxy_endpoint_preserves_response_headers() {
        let router = build_router_with_mocks(
            target_servers(),
            |mock| {
                mock.expect_execute()
                    .withf(move |req| {
                        req.method == RequestMethod::Get
                            && req.url == "http://target.com/"
                            && req.body == Bytes::new()
                    })
                    .returning(|_| {
                        let mut headers = RequestHeaders::default();
                        headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
                        headers.insert("Content-Type".to_string(), "application/json".to_string());

                        Ok(HttpClientResponse {
                            status: 200,
                            headers,
                            body: Bytes::from("{}"),
                        })
                    });
            },
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response.headers().get("x-custom-header").unwrap(),
            "custom-value"
        );
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
    }

    #[tokio::test]
    async fn proxy_endpoint_preserves_response_body() {
        let expected_body = r#"{"data": "test"}"#;
        let router = build_router_with_mocks(
            target_servers(),
            |mock| {
                mock.expect_execute()
                    .withf(move |req| {
                        req.method == RequestMethod::Get
                            && req.url == "http://target.com/"
                            && req.body == Bytes::new()
                    })
                    .returning(move |_| {
                        Ok(HttpClientResponse {
                            status: 200,
                            headers: RequestHeaders::default(),
                            body: Bytes::from(expected_body),
                        })
                    });
            },
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();

        assert_eq!(body_bytes, Bytes::from(expected_body));
    }

    #[tokio::test]
    async fn proxy_endpoint_sends_request_headers() {
        let router = build_router_with_mocks(
            target_servers(),
            |mock| {
                mock.expect_execute()
                    .withf(move |req| {
                        req.method == RequestMethod::Get
                            && req.url == "http://target.com/"
                            && req.body == Bytes::new()
                            && req.headers.get("authorization") == Some(&"Bearer token".to_string())
                            && req.headers.get("content-type")
                                == Some(&"application/json".to_string())
                    })
                    .returning(|_| {
                        Ok(HttpClientResponse {
                            status: 200,
                            headers: RequestHeaders::default(),
                            body: Bytes::new(),
                        })
                    });
            },
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Authorization", "Bearer token")
                    .header("Content-Type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn proxy_endpoint_sends_request_body() {
        let request_body = r#"{"key": "value"}"#;
        let router = build_router_with_mocks(
            target_servers(),
            |mock| {
                mock.expect_execute()
                    .withf(move |req| {
                        req.method == RequestMethod::Post
                            && req.url == "http://target.com/"
                            && req.body == Bytes::from(request_body)
                    })
                    .returning(|_| {
                        Ok(HttpClientResponse {
                            status: 200,
                            headers: RequestHeaders::default(),
                            body: Bytes::new(),
                        })
                    });
            },
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/")
                    .method(Method::POST)
                    .body(Body::from(request_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn proxy_endpoint_sends_request_path() {
        let router = build_router_with_mocks(
            target_servers(),
            |mock| {
                mock.expect_execute()
                    .withf(|req| req.url == "http://target.com/api/users")
                    .returning(|_| {
                        Ok(HttpClientResponse {
                            status: 200,
                            headers: RequestHeaders::default(),
                            body: Bytes::new(),
                        })
                    });
            },
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/users")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn proxy_endpoint_sends_request_method() {
        for (method, method_str) in [
            (Method::GET, "GET"),
            (Method::POST, "POST"),
            (Method::PUT, "PUT"),
            (Method::DELETE, "DELETE"),
            (Method::PATCH, "PATCH"),
        ] {
            let router = build_router_with_mocks(
                target_servers(),
                |mock| {
                    let expected_method = method_str.to_string();
                    mock.expect_execute()
                        .withf(move |req| req.method.to_string() == expected_method)
                        .returning(|_| {
                            Ok(HttpClientResponse {
                                status: 200,
                                headers: RequestHeaders::default(),
                                body: Bytes::new(),
                            })
                        });
                },
                first_one_select_server_mock(),
            );

            let response = router
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri("/")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(
                response.status(),
                StatusCode::OK,
                "Failed for method {}",
                method_str
            );
        }
    }

    #[tokio::test]
    async fn proxy_endpoint_handles_network_error() {
        let router = build_router_with_mocks(
            target_servers(),
            |mock| {
                mock.expect_execute()
                    .returning(|_| Err(HttpClientError::Network("Connection refused".to_string())));
            },
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn proxy_endpoint_handles_timeout_error() {
        let router = build_router_with_mocks(
            target_servers(),
            |mock| {
                mock.expect_execute()
                    .returning(|_| Err(HttpClientError::Timeout));
            },
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    }

    #[tokio::test]
    async fn proxy_endpoint_handles_invalid_request_error() {
        let router = build_router_with_mocks(
            target_servers(),
            |mock| {
                mock.expect_execute()
                    .returning(|_| Err(HttpClientError::InvalidRequest("Bad URL".to_string())));
            },
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn proxy_endpoint_includes_request_id_in_response() {
        let router = build_router_with_mocks(
            target_servers(),
            build_success_http_client_mock(),
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let request_id = response.headers().get(X_REQUEST_ID);
        assert!(request_id.is_some());
        assert!(!request_id.unwrap().to_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn proxy_endpoint_propagates_existing_request_id() {
        let custom_request_id = "custom-12345";
        let router = build_router_with_mocks(
            target_servers(),
            build_success_http_client_mock(),
            first_one_select_server_mock(),
        );

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(X_REQUEST_ID.as_str(), custom_request_id)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let request_id = response.headers().get(X_REQUEST_ID).unwrap();
        assert_eq!(request_id.to_str().unwrap(), custom_request_id);
    }

    #[tokio::test]
    async fn converts_domain_response_to_http_response() {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        let http_client_response = HttpClientResponse {
            status: 200,
            headers: headers.into(),
            body: Bytes::from(r#"{"key":"value"}"#),
        };

        let response: AxumResponse<Body> = http_client_response.into();
        assert_eq!(response.status(), StatusCode::OK);

        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body_bytes, r#"{"key":"value"}"#.as_bytes());
    }

    #[test]
    fn converts_domain_errors_to_http_error() {
        let network_error = HttpClientError::Network("Connection refused".into());
        let invalid_request = HttpClientError::InvalidRequest("Bad data".into());
        let timeout = HttpClientError::Timeout;

        let (status, msg): (StatusCode, &str) = network_error.into();
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(msg, "Network error");

        let (status, msg): (StatusCode, &str) = invalid_request.into();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(msg, "Invalid request");

        let (status, msg): (StatusCode, &str) = timeout.into();
        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(msg, "Timeout");
    }

    #[tokio::test]
    async fn proxy_endpoint_handles_no_one_is_alive_error() {
        let router = build_router_with_mocks(
            target_servers(),
            build_success_http_client_mock(),
            |select_server_mock, _| {
                select_server_mock
                    .expect_execute()
                    .returning(move |_| Err(SelectServerError::NoOneIsAlive));
            },
        );

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
