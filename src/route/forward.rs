use crate::ServerState;
use crate::request_id::UNKNOWN_REQUEST_ID;
use axum::body::Body;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{extract::State, http::Request};
use tower_http::request_id::RequestId;
use tracing::{error, info};

pub(crate) async fn forward(
    State(state): State<ServerState>,
    request: Request<Body>,
) -> impl IntoResponse {
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .and_then(|id| id.header_value().to_str().ok())
        .unwrap_or(UNKNOWN_REQUEST_ID);

    match state
        .client
        .send(
            state.target_servers_base_url.clone(),
            request_id.to_string(),
        )
        .await
    {
        Ok(body) => {
            info!("Request forwarded to {}", state.target_servers_base_url);
            body.into_response()
        }
        Err(err) => {
            error!("Forwarding request failed: {err}");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ServerState;
    use crate::http_client::alpha_client::MockAlphaClient;
    use crate::route::forward::forward;
    use axum::body::{Body, Bytes};
    use axum::extract::State;
    use axum::http::Request;
    use axum::response::IntoResponse;
    use http::HeaderValue;
    use mockall::predicate::{always, function};
    use reqwest::StatusCode;
    use std::sync::Arc;
    use tower_http::request_id::RequestId;
    use uuid::Uuid;

    fn make_request() -> Request<Body> {
        let mut req = Request::new(Body::empty());
        let uuid = Uuid::new_v4().to_string();
        let header_val = HeaderValue::from_str(&uuid).unwrap();

        req.extensions_mut().insert(RequestId::new(header_val));
        req
    }

    #[tokio::test]
    async fn should_forward_the_request() {
        let mut mock_alpha_client = MockAlphaClient::default();

        mock_alpha_client
            .expect_send()
            .returning(move |_, _| Ok(String::from("FORWARDED")));

        let result = forward(
            State(ServerState {
                client: Arc::new(mock_alpha_client),
                target_servers_base_url: String::from("http://localhost:3000/"),
            }),
            make_request(),
        )
        .await;

        let response = result.into_response();
        let body = response.into_body();
        let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        assert_eq!(body_bytes, Bytes::from_static(b"FORWARDED"));
    }

    #[tokio::test]
    async fn should_enrich_the_forwarded_request_with_request_id() {
        let mut mock_alpha_client = MockAlphaClient::default();

        mock_alpha_client
            .expect_send()
            .with(always(), function(|s: &String| s != "unknown"))
            .returning(move |_, _| Ok(String::from("FORWARDED")));

        forward(
            State(ServerState {
                client: Arc::new(mock_alpha_client),
                target_servers_base_url: String::from("http://localhost:3000/"),
            }),
            make_request(),
        )
        .await;
    }

    #[tokio::test]
    async fn should_fail_forwarding_the_request() {
        let mut mock_alpha_client = MockAlphaClient::default();

        mock_alpha_client
            .expect_send()
            .returning(move |_, _| Err("Mocked error".into()));

        let result = forward(
            State(ServerState {
                client: Arc::new(mock_alpha_client),
                target_servers_base_url: String::from("http://localhost:3000/"),
            }),
            make_request(),
        )
        .await;

        let response = result.into_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY)
    }
}
