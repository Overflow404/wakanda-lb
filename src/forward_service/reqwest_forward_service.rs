use std::collections::HashMap;

use async_trait::async_trait;
use axum::response::IntoResponse;
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use tracing::info;

use crate::forward_service::{
    forward_service::ForwardService,
    forward_service_request::{
        ForwardServiceRequest, ForwardServiceRequestError, ForwardServiceRequestHeaders,
        ForwardServiceRequestHttpMethod,
    },
    forward_service_response::{ForwardServiceError, ForwardServiceResponse},
};

#[derive(Clone)]
pub struct ReqwestForwardService {
    client: reqwest::Client,
}

impl ReqwestForwardService {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build reqwest client"),
        }
    }

    #[cfg(test)]
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl ForwardServiceRequestHttpMethod {
    fn to_reqwest(&self) -> reqwest::Method {
        match self {
            ForwardServiceRequestHttpMethod::Get => reqwest::Method::GET,
            ForwardServiceRequestHttpMethod::Post => reqwest::Method::POST,
            ForwardServiceRequestHttpMethod::Put => reqwest::Method::PUT,
            ForwardServiceRequestHttpMethod::Delete => reqwest::Method::DELETE,
            ForwardServiceRequestHttpMethod::Patch => reqwest::Method::PATCH,
        }
    }
}

#[async_trait]
impl ForwardService for ReqwestForwardService {
    async fn execute(
        &self,
        target_url: &str,
        request: ForwardServiceRequest,
    ) -> Result<ForwardServiceResponse, ForwardServiceError> {
        let url = format!("{}{}", target_url, request.path);
        info!("New url is {}", url);

        let req_builder = self
            .client
            .request(request.method.to_reqwest(), &url)
            .headers(request.headers.into())
            .body(request.body.clone());

        let response = req_builder
            .send()
            .await
            .map_err(ForwardServiceError::from)?;

        let status = response.status().as_u16();

        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
            .collect();

        let body = response
            .bytes()
            .await
            .map_err(|e| ForwardServiceError::Network(e.to_string()))?;

        Ok(ForwardServiceResponse {
            status,
            headers,
            body,
        })
    }
}

impl From<reqwest::Error> for ForwardServiceError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ForwardServiceError::Timeout
        } else if err.is_connect() || err.is_request() {
            ForwardServiceError::Network(err.to_string())
        } else {
            ForwardServiceError::InvalidRequest(err.to_string())
        }
    }
}

impl From<HeaderMap> for ForwardServiceRequestHeaders {
    fn from(headers: HeaderMap) -> Self {
        let map = headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
            .collect();
        ForwardServiceRequestHeaders(map)
    }
}

impl From<ForwardServiceRequestHeaders> for HeaderMap {
    fn from(h: ForwardServiceRequestHeaders) -> Self {
        let mut header_map = HeaderMap::new();
        for (k, v) in h.0 {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(&v),
            ) {
                header_map.insert(name, value);
            }
        }
        header_map
    }
}

impl IntoResponse for ForwardServiceRequestError {
    fn into_response(self) -> axum::response::Response {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

impl TryFrom<&Method> for ForwardServiceRequestHttpMethod {
    type Error = ForwardServiceRequestError;

    fn try_from(value: &Method) -> Result<Self, Self::Error> {
        match *value {
            Method::GET => Ok(ForwardServiceRequestHttpMethod::Get),
            Method::POST => Ok(ForwardServiceRequestHttpMethod::Post),
            Method::PUT => Ok(ForwardServiceRequestHttpMethod::Put),
            Method::DELETE => Ok(ForwardServiceRequestHttpMethod::Delete),
            Method::PATCH => Ok(ForwardServiceRequestHttpMethod::Patch),
            _ => Err(ForwardServiceRequestError::UnsupportedMethod(
                value.to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::forward_service::forward_service_request::ForwardServiceRequestHeaders;

    use super::*;
    use bytes::Bytes;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_successful_get_request() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("PONG", "application/text"))
            .mount(&mock_server)
            .await;

        let client = ReqwestForwardService::new();
        let request = ForwardServiceRequest {
            method: ForwardServiceRequestHttpMethod::Get,
            path: "/health".to_string(),
            headers: ForwardServiceRequestHeaders::default(),
            body: Bytes::new(),
        };

        let response = client.execute(&mock_server.uri(), request).await.unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.len() > 0);
    }

    #[tokio::test]
    async fn test_post_with_headers_and_body() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/data"))
            .and(header("Authorization", "Bearer secret"))
            .and(header("Content-Type", "text/plain"))
            .respond_with(
                ResponseTemplate::new(201)
                    .insert_header("X-Request-Id", "12345")
                    .set_body_string("Created"),
            )
            .mount(&mock_server)
            .await;

        let client = ReqwestForwardService::new();
        let request = ForwardServiceRequest {
            method: ForwardServiceRequestHttpMethod::Post,
            path: "/api/data".to_string(),
            headers: ForwardServiceRequestHeaders::from([
                ("Authorization".to_string(), "Bearer secret".to_string()),
                ("Content-Type".to_string(), "text/plain".to_string()),
            ]),
            body: Bytes::from(r#"{"key":"value"}"#),
        };

        let response = client.execute(&mock_server.uri(), request).await.unwrap();

        assert_eq!(response.status, 201);
        assert_eq!(response.headers.get("x-request-id").unwrap(), "12345");
        assert_eq!(response.body, Bytes::from("Created"));
    }

    #[tokio::test]
    async fn test_network_error() {
        let client = ReqwestForwardService::new();
        let request = ForwardServiceRequest {
            method: ForwardServiceRequestHttpMethod::Get,
            path: "/health".to_string(),
            headers: ForwardServiceRequestHeaders::default(),
            body: Bytes::new(),
        };

        let result = client.execute("http://localhost:9999", request).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ForwardServiceError::Network(_)
        ));
    }

    #[tokio::test]
    async fn test_timeout() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(60)))
            .mount(&mock_server)
            .await;

        let client = ReqwestForwardService::with_client(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(100))
                .build()
                .unwrap(),
        );

        let request = ForwardServiceRequest {
            method: ForwardServiceRequestHttpMethod::Get,
            path: "/slow".to_string(),
            headers: ForwardServiceRequestHeaders::default(),
            body: Bytes::new(),
        };

        let result = client.execute(&mock_server.uri(), request).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ForwardServiceError::Timeout));
    }

    #[tokio::test]
    async fn test_all_http_methods() {
        let mock_server = MockServer::start().await;

        for (method_enum, method_str) in [
            (ForwardServiceRequestHttpMethod::Get, "GET"),
            (ForwardServiceRequestHttpMethod::Post, "POST"),
            (ForwardServiceRequestHttpMethod::Put, "PUT"),
            (ForwardServiceRequestHttpMethod::Delete, "DELETE"),
            (ForwardServiceRequestHttpMethod::Patch, "PATCH"),
        ] {
            Mock::given(method(method_str))
                .and(path("/health"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let client = ReqwestForwardService::new();
            let request = ForwardServiceRequest {
                method: method_enum,
                path: "/health".to_string(),
                headers: ForwardServiceRequestHeaders::default(),
                body: Bytes::new(),
            };

            let response = client.execute(&mock_server.uri(), request).await.unwrap();
            assert_eq!(response.status, 200);
        }
    }

    #[tokio::test]
    async fn test_response_headers_parsing() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("X-Custom-Header", "custom-value")
                    .insert_header("Content-Type", "text/plain")
                    .set_body_string("{}"),
            )
            .mount(&mock_server)
            .await;

        let client = ReqwestForwardService::new();
        let request = ForwardServiceRequest {
            method: ForwardServiceRequestHttpMethod::Get,
            path: "/".to_string(),
            headers: ForwardServiceRequestHeaders::default(),
            body: Bytes::new(),
        };

        let response = client.execute(&mock_server.uri(), request).await.unwrap();

        assert_eq!(
            response.headers.get("x-custom-header").unwrap(),
            "custom-value"
        );
        assert_eq!(response.headers.get("content-type").unwrap(), "text/plain");
    }
}
