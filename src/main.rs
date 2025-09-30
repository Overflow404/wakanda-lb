mod cli_arguments;
pub(crate) mod forward_service;

use crate::cli_arguments::CliArguments;
use crate::forward_service::forward_service::ForwardService;
use crate::forward_service::forward_service_request::ForwardServiceRequest;
use crate::forward_service::forward_service_response::ForwardServiceError;
use crate::forward_service::simple_forward_service::SimpleForwardService;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::{Router, routing::get};
use clap::Parser;
use http::{HeaderName, StatusCode};
use std::sync::Arc;
use tower_http::request_id::{
    MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer,
};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::{error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

pub(crate) const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
pub(crate) const UNKNOWN_REQUEST_ID: &str = "unknown";

#[derive(Clone, Default)]
pub(crate) struct AlphaRequestId {}

impl MakeRequestId for AlphaRequestId {
    fn make_request_id<B>(&mut self, _: &Request<B>) -> Option<RequestId> {
        let request_id = Uuid::new_v4().to_string().parse().unwrap();

        Some(RequestId::new(request_id))
    }
}

#[derive(Clone)]
pub(crate) struct ServerState {
    forward_service: Arc<dyn ForwardService + Send + Sync>,
    target_servers_base_url: String,
}

async fn health_endpoint() -> impl IntoResponse {
    info!("Health check executed");
    "PONG"
}

async fn forward_endpoint(
    State(state): State<ServerState>,
    request: Request<Body>,
) -> impl IntoResponse {
    info!("Executing request forwarding");
    let forward_service = state.forward_service;

    let (parts, body) = request.into_parts();

    let bytes = match to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(err) => {
            error!("Failed to read body: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read body while forwarding",
            )
                .into_response();
        }
    };

    let response = forward_service
        .execute(
            &state.target_servers_base_url,
            ForwardServiceRequest {
                method: (&parts.method).into(),
                path: parts.uri.path().to_string(),
                headers: parts.headers.into(),
                body: bytes,
            },
        )
        .await;

    match response {
        Ok(forward_resp) => {
            let mut response = Response::builder()
                .status(StatusCode::from_u16(forward_resp.status).unwrap_or(StatusCode::OK));

            for (k, v) in forward_resp.headers.iter() {
                response = response.header(k, v);
            }

            response
                .body(Body::from(forward_resp.body))
                .unwrap()
                .into_response()
        }
        Err(e) => {
            let (status, msg) = match e {
                ForwardServiceError::Network(_) => (StatusCode::BAD_GATEWAY, "Network error"),
                ForwardServiceError::InvalidRequest(_) => {
                    (StatusCode::BAD_REQUEST, "Invalid request")
                }
                ForwardServiceError::Timeout => (StatusCode::GATEWAY_TIMEOUT, "Timeout"),
                ForwardServiceError::ServerError { status } => (
                    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    "Server error",
                ),
            };

            (status, msg).into_response()
        }
    }
}

pub(crate) fn router(server_state: ServerState) -> Router {
    Router::new()
        .route("/health", get(health_endpoint))
        .route("/{*path}", any(forward_endpoint))
        .route("/", any(forward_endpoint))
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

    let forward_service = Arc::new(SimpleForwardService::new());
    let target_servers_base_url = String::from(args.target_servers_base_url);

    let state = ServerState {
        forward_service: forward_service,
        target_servers_base_url,
    };

    axum::serve(tcp_listener, router(state)).await.unwrap();
}

#[cfg(test)]
mod tests {

    use crate::forward_service::forward_service::MockForwardService;
    use crate::forward_service::forward_service_response::{
        ForwardServiceError, ForwardServiceResponse,
    };
    use crate::{ServerState, X_REQUEST_ID, router};
    use axum::body::{Body, Bytes};
    use axum::http::{Method, Request, StatusCode};
    use mockall::predicate::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn build_router_with_mock(
        url: String,
        setup: impl FnOnce(&mut MockForwardService),
    ) -> axum::Router {
        let mut mock = MockForwardService::default();
        setup(&mut mock);

        router(ServerState {
            forward_service: Arc::new(mock),
            target_servers_base_url: url,
        })
    }

    fn build_success_mock() -> impl FnOnce(&mut MockForwardService) {
        |mock: &mut MockForwardService| {
            mock.expect_execute().returning(|_, _| {
                Ok(ForwardServiceResponse {
                    status: 200,
                    headers: HashMap::new(),
                    body: Bytes::from("OK"),
                })
            });
        }
    }

    #[tokio::test]
    async fn health_endpoint_returns_pong() {
        let router =
            build_router_with_mock(String::from("http://localhost:3000"), build_success_mock());

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

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();

        assert_eq!(body_bytes, Bytes::from_static(b"PONG"));
    }

    #[tokio::test]
    async fn health_endpoint_includes_request_id() {
        let router =
            build_router_with_mock(String::from("http://localhost:3000"), build_success_mock());

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response.headers().get(X_REQUEST_ID).is_some());
    }

    #[tokio::test]
    async fn forward_endpoint_forwards_get_request() {
        let target_url = String::from("http://target.com");
        let router = build_router_with_mock(target_url.clone(), |mock| {
            mock.expect_execute()
                .with(eq(target_url), always())
                .times(1)
                .returning(|_, _| {
                    Ok(ForwardServiceResponse {
                        status: 200,
                        headers: HashMap::new(),
                        body: Bytes::from("Success"),
                    })
                });
        });

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
    async fn forward_endpoint_preserves_response_status() {
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            mock.expect_execute().returning(|_, _| {
                Ok(ForwardServiceResponse {
                    status: 201,
                    headers: HashMap::new(),
                    body: Bytes::from("Created"),
                })
            });
        });

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn forward_endpoint_preserves_response_headers() {
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            mock.expect_execute().returning(|_, _| {
                let mut headers = HashMap::new();
                headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
                headers.insert("Content-Type".to_string(), "application/json".to_string());

                Ok(ForwardServiceResponse {
                    status: 200,
                    headers,
                    body: Bytes::from("{}"),
                })
            });
        });

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
    async fn forward_endpoint_preserves_response_body() {
        let expected_body = r#"{"data": "test"}"#;
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            let body_clone = expected_body.to_string();
            mock.expect_execute().returning(move |_, _| {
                Ok(ForwardServiceResponse {
                    status: 200,
                    headers: HashMap::new(),
                    body: Bytes::from(body_clone.clone()),
                })
            });
        });

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
    async fn forward_endpoint_forwards_request_headers() {
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            mock.expect_execute()
                .withf(|_, req| {
                    req.headers.get("authorization") == Some(&"Bearer token".to_string())
                        && req.headers.get("content-type") == Some(&"application/json".to_string())
                })
                .returning(|_, _| {
                    Ok(ForwardServiceResponse {
                        status: 200,
                        headers: HashMap::new(),
                        body: Bytes::new(),
                    })
                });
        });

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
    async fn forward_endpoint_forwards_request_body() {
        let request_body = r#"{"key": "value"}"#;
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            let expected_body = request_body.to_string();
            mock.expect_execute()
                .withf(move |_, req| req.body == Bytes::from(expected_body.clone()))
                .returning(|_, _| {
                    Ok(ForwardServiceResponse {
                        status: 200,
                        headers: HashMap::new(),
                        body: Bytes::new(),
                    })
                });
        });

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
    async fn forward_endpoint_forwards_request_path() {
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            mock.expect_execute()
                .withf(|_, req| req.path == "/api/users")
                .returning(|_, _| {
                    Ok(ForwardServiceResponse {
                        status: 200,
                        headers: HashMap::new(),
                        body: Bytes::new(),
                    })
                });
        });

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
    async fn forward_endpoint_forwards_request_method() {
        for (method, method_str) in [
            (Method::GET, "GET"),
            (Method::POST, "POST"),
            (Method::PUT, "PUT"),
            (Method::DELETE, "DELETE"),
            (Method::PATCH, "PATCH"),
        ] {
            let router = build_router_with_mock(String::from("http://target.com"), |mock| {
                let expected_method = method_str.to_string();
                mock.expect_execute()
                    .withf(move |_, req| req.method.to_string() == expected_method)
                    .returning(|_, _| {
                        Ok(ForwardServiceResponse {
                            status: 200,
                            headers: HashMap::new(),
                            body: Bytes::new(),
                        })
                    });
            });

            let response = router
                .oneshot(
                    Request::builder()
                        .method(method.clone())
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
    async fn forward_endpoint_handles_network_error() {
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            mock.expect_execute().returning(|_, _| {
                Err(ForwardServiceError::Network(
                    "Connection refused".to_string(),
                ))
            });
        });

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn forward_endpoint_handles_timeout_error() {
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            mock.expect_execute()
                .returning(|_, _| Err(ForwardServiceError::Timeout));
        });

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    }

    #[tokio::test]
    async fn forward_endpoint_handles_invalid_request_error() {
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            mock.expect_execute()
                .returning(|_, _| Err(ForwardServiceError::InvalidRequest("Bad URL".to_string())));
        });

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn forward_endpoint_handles_server_error() {
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            mock.expect_execute()
                .returning(|_, _| Err(ForwardServiceError::ServerError { status: 500 }));
        });

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn forward_endpoint_handles_custom_server_error_status() {
        let router = build_router_with_mock(String::from("http://target.com"), |mock| {
            mock.expect_execute()
                .returning(|_, _| Err(ForwardServiceError::ServerError { status: 503 }));
        });

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn forward_endpoint_includes_request_id_in_response() {
        let router =
            build_router_with_mock(String::from("http://target.com"), build_success_mock());

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let request_id = response.headers().get(X_REQUEST_ID);
        assert!(request_id.is_some());
        assert!(!request_id.unwrap().to_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn forward_endpoint_propagates_existing_request_id() {
        let custom_request_id = "custom-12345";
        let router =
            build_router_with_mock(String::from("http://target.com"), build_success_mock());

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
}
