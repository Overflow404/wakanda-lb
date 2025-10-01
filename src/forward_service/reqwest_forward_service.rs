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

    #[allow(dead_code)]
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ForwardService for ReqwestForwardService {
    async fn execute(
        &self,
        request: ForwardServiceRequest,
    ) -> Result<ForwardServiceResponse, ForwardServiceError> {
        info!("Forwarding {:#?}", request);

        let reqwuest_builder = self
            .client
            .request(request.method.into(), request.url)
            .headers(request.headers.into())
            .body(request.body.clone());

        let reqwest_response = reqwuest_builder
            .send()
            .await
            .map_err(ForwardServiceError::from)?;

        let http_status = reqwest_response.status().as_u16();

        let headers: ForwardServiceRequestHeaders = reqwest_response.headers().into();

        let body = reqwest_response
            .bytes()
            .await
            .map_err(|e| ForwardServiceError::Network(e.to_string()))?;

        Ok(ForwardServiceResponse {
            status: http_status,
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

impl From<&HeaderMap> for ForwardServiceRequestHeaders {
    fn from(headers: &HeaderMap) -> Self {
        let map = headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
            .collect();
        ForwardServiceRequestHeaders(map)
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
        for (k, v) in h.iter() {
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

impl From<ForwardServiceRequestHttpMethod> for reqwest::Method {
    fn from(value: ForwardServiceRequestHttpMethod) -> Self {
        match value {
            ForwardServiceRequestHttpMethod::Get => reqwest::Method::GET,
            ForwardServiceRequestHttpMethod::Post => reqwest::Method::POST,
            ForwardServiceRequestHttpMethod::Put => reqwest::Method::PUT,
            ForwardServiceRequestHttpMethod::Delete => reqwest::Method::DELETE,
            ForwardServiceRequestHttpMethod::Patch => reqwest::Method::PATCH,
        }
    }
}

#[cfg(test)]
mod tests {

    // impl From<reqwest::Error> for ForwardServiceError
    // impl From<&HeaderMap> for ForwardServiceRequestHeaders
    // impl From<HeaderMap> for ForwardServiceRequestHeaders
    // impl From<ForwardServiceRequestHeaders> for HeaderMap
    // impl IntoResponse for ForwardServiceRequestError
    // impl TryFrom<&Method> for ForwardServiceRequestHttpMethod
    // impl From<ForwardServiceRequestHttpMethod> for reqwest::Method
}
