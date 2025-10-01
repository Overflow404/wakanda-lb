mod cli_arguments;
pub(crate) mod forward_service;
mod request_id;

use crate::cli_arguments::CliArguments;
use crate::forward_service::forward_service::ForwardService;
use crate::forward_service::forward_service_request::{
    ForwardServiceRequest, ForwardServiceRequestHttpMethod,
};
use crate::forward_service::forward_service_response::{
    ForwardServiceError, ForwardServiceResponse,
};
use crate::forward_service::reqwest_forward_service::ReqwestForwardService;
use crate::request_id::{LoadBalancerRequestId, UNKNOWN_REQUEST_ID, X_REQUEST_ID};
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::{Router, routing::get};
use clap::Parser;
use http::StatusCode;
use std::sync::Arc;
use tower_http::request_id::{PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::{error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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

    let (parts, body) = request.into_parts();

    let url = format!(
        "{}{}",
        state.target_servers_base_url,
        parts.uri.path().to_string()
    );

    let headers = parts.headers.into();

    let body = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(error) => {
            error!("Failed body conversion into bytes: {}", error);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let method: ForwardServiceRequestHttpMethod = match (&parts.method).try_into() {
        Ok(method) => method,
        Err(error) => {
            error!("{}", error.to_string());
            return error.into_response();
        }
    };

    let result = state
        .forward_service
        .execute(ForwardServiceRequest {
            method,
            headers,
            body,
            url,
        })
        .await;

    match result {
        Ok(forward_service_response) => forward_service_response.into(),
        Err(error) => {
            let (status, error) = error.into();
            error!("Error: {} Status: {}", error, status);

            (status, error).into_response()
        }
    }
}

impl From<ForwardServiceResponse> for Response<Body> {
    fn from(value: ForwardServiceResponse) -> Self {
        let mut response = Response::builder()
            .status(StatusCode::from_u16(value.status).unwrap_or(StatusCode::OK));

        for (k, v) in value.headers.iter() {
            response = response.header(k, v);
        }

        response.body(Body::from(value.body)).unwrap()
    }
}

impl From<ForwardServiceError> for (StatusCode, &str) {
    fn from(value: ForwardServiceError) -> Self {
        match value {
            ForwardServiceError::Network(_) => (StatusCode::BAD_GATEWAY, "Network error"),
            ForwardServiceError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, "Invalid request"),
            ForwardServiceError::Timeout => (StatusCode::GATEWAY_TIMEOUT, "Timeout"),
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

    let forward_service = Arc::new(ReqwestForwardService::new());
    let target_servers_base_url = String::from(args.target_servers_base_url);

    let state = ServerState {
        forward_service,
        target_servers_base_url,
    };

    axum::serve(tcp_listener, router(state)).await.unwrap();
}

#[cfg(test)]
mod tests {

    use crate::forward_service::forward_service::MockForwardService;
    use crate::forward_service::forward_service_request::ForwardServiceRequestHeaders;
    use crate::forward_service::forward_service_response::{
        ForwardServiceError, ForwardServiceResponse,
    };
    use crate::{ServerState, X_REQUEST_ID, router};
    use axum::body::{Body, Bytes};
    use axum::http::{Method, Request, StatusCode};
    use axum::response::Response;
    use http::{HeaderMap, HeaderValue};
    use mockall::predicate::*;
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
            mock.expect_execute().returning(|_| {
                Ok(ForwardServiceResponse {
                    status: 200,
                    headers: ForwardServiceRequestHeaders::default(),
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
                .with(always()) //TODO proper object
                .times(1)
                .returning(|_| {
                    Ok(ForwardServiceResponse {
                        status: 200,
                        headers: ForwardServiceRequestHeaders::default(),
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
            mock.expect_execute().returning(|_| {
                Ok(ForwardServiceResponse {
                    status: 201,
                    headers: ForwardServiceRequestHeaders::default(),
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
            mock.expect_execute().returning(|_| {
                let mut headers = ForwardServiceRequestHeaders::default();
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
            mock.expect_execute().returning(move |_| {
                Ok(ForwardServiceResponse {
                    status: 200,
                    headers: ForwardServiceRequestHeaders::default(),
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
                .withf(|req| {
                    req.headers.get("authorization") == Some(&"Bearer token".to_string())
                        && req.headers.get("content-type") == Some(&"application/json".to_string())
                })
                .returning(|_| {
                    Ok(ForwardServiceResponse {
                        status: 200,
                        headers: ForwardServiceRequestHeaders::default(),
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
                .withf(move |req| req.body == Bytes::from(expected_body.clone()))
                .returning(|_| {
                    Ok(ForwardServiceResponse {
                        status: 200,
                        headers: ForwardServiceRequestHeaders::default(),
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
                .withf(|req| req.url == "http://target.com/api/users")
                .returning(|_| {
                    Ok(ForwardServiceResponse {
                        status: 200,
                        headers: ForwardServiceRequestHeaders::default(),
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
                    .withf(move |req| req.method.to_string() == expected_method)
                    .returning(|_| {
                        Ok(ForwardServiceResponse {
                            status: 200,
                            headers: ForwardServiceRequestHeaders::default(),
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
            mock.expect_execute().returning(|_| {
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
                .returning(|_| Err(ForwardServiceError::Timeout));
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
                .returning(|_| Err(ForwardServiceError::InvalidRequest("Bad URL".to_string())));
        });

        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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

    #[tokio::test]
    async fn test_forward_service_response_to_http_response() {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        let forward_response = ForwardServiceResponse {
            status: 200,
            headers: headers.into(),
            body: Bytes::from(r#"{"key":"value"}"#),
        };

        let response: Response<Body> = forward_response.into();
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
    fn test_forward_service_error_to_status_and_string() {
        let network_error = ForwardServiceError::Network("Connection refused".into());
        let invalid_request = ForwardServiceError::InvalidRequest("Bad data".into());
        let timeout = ForwardServiceError::Timeout;

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
}
