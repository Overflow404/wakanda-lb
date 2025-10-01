use async_trait::async_trait;
use axum::response::IntoResponse;
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use tracing::info;

use crate::forward_service::{
    forward_service::ForwardService,
    forward_service_error::ForwardServiceErrorChecker,
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
    #[allow(dead_code)]
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for ReqwestForwardService {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build reqwest client"),
        }
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

impl ForwardServiceErrorChecker for reqwest::Error {
    fn is_timeout(&self) -> bool {
        self.is_timeout()
    }

    fn is_connect(&self) -> bool {
        self.is_connect()
    }

    fn is_request(&self) -> bool {
        self.is_request()
    }

    fn error_string(&self) -> String {
        self.to_string()
    }
}

impl<T: ForwardServiceErrorChecker> From<T> for ForwardServiceError {
    fn from(err: T) -> Self {
        if err.is_timeout() {
            ForwardServiceError::Timeout
        } else if err.is_connect() || err.is_request() {
            ForwardServiceError::Network(err.error_string())
        } else {
            ForwardServiceError::InvalidRequest(err.error_string())
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
    use axum::response::IntoResponse;
    use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};

    use crate::forward_service::{
        forward_service_error::MockForwardServiceErrorChecker,
        forward_service_request::{
            ForwardServiceRequestError, ForwardServiceRequestHeaders,
            ForwardServiceRequestHttpMethod,
        },
        forward_service_response::ForwardServiceError,
    };

    #[test]
    fn converts_reqwest_errors_into_domain_variants() {
        let mut mock = MockForwardServiceErrorChecker::new();
        mock.expect_is_timeout().return_const(true);
        let result: ForwardServiceError = mock.into();
        assert!(matches!(result, ForwardServiceError::Timeout));

        mock = MockForwardServiceErrorChecker::new();
        mock.expect_is_timeout().return_const(false);
        mock.expect_is_connect().return_const(true);
        mock.expect_error_string()
            .return_const("connect error".to_string());
        let result: ForwardServiceError = mock.into();
        assert!(matches!(result, ForwardServiceError::Network(_)));

        mock = MockForwardServiceErrorChecker::new();
        mock.expect_is_timeout().return_const(false);
        mock.expect_is_connect().return_const(false);
        mock.expect_is_request().return_const(true);
        mock.expect_error_string()
            .return_const("request error".to_string());
        let result: ForwardServiceError = mock.into();
        assert!(matches!(result, ForwardServiceError::Network(_)));

        mock = MockForwardServiceErrorChecker::new();
        mock.expect_is_timeout().return_const(false);
        mock.expect_is_connect().return_const(false);
        mock.expect_is_request().return_const(false);
        mock.expect_error_string()
            .return_const("other error".to_string());
        let result: ForwardServiceError = mock.into();
        assert!(matches!(result, ForwardServiceError::InvalidRequest(_)));
    }

    #[test]
    fn extracts_only_valid_headers_from_header_map() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            HeaderName::from_static("x-custom-header"),
            HeaderValue::from_static("custom-value"),
        );
        headers.insert(
            HeaderName::from_static("invalid-header"),
            HeaderValue::from_bytes(&[0xFF, 0xFE]).unwrap(),
        );

        let result_borrowed: ForwardServiceRequestHeaders = (&headers).into();
        let result_owned: ForwardServiceRequestHeaders = headers.into();

        assert_eq!(result_borrowed.0.len(), 2);
        assert_eq!(result_owned.0.len(), 2);

        assert_eq!(
            result_borrowed.get("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(
            result_owned.get("content-type"),
            Some(&"application/json".to_string())
        );

        assert_eq!(
            result_borrowed.get("x-custom-header"),
            Some(&"custom-value".to_string())
        );
        assert_eq!(
            result_owned.get("x-custom-header"),
            Some(&"custom-value".to_string())
        );

        assert_eq!(result_borrowed.get("invalid-header"), None);
        assert_eq!(result_owned.get("invalid-header"), None);
    }

    #[test]
    fn builds_header_map_from_valid_domain_headers() {
        let mut forward_service_request_headers = ForwardServiceRequestHeaders::default();
        forward_service_request_headers
            .insert("content-type".to_string(), "application/json".to_string());
        forward_service_request_headers
            .insert("x-custom-header".to_string(), "custom-value".to_string());

        let result: HeaderMap = forward_service_request_headers.into();
        assert_eq!(
            result.get("content-type"),
            Some(&HeaderValue::from_static("application/json"))
        );

        assert_eq!(
            result.get("x-custom-header"),
            Some(&HeaderValue::from_static("custom-value"))
        );
    }

    #[test]
    fn converts_domain_request_error_into_axum_response() {
        let error =
            ForwardServiceRequestError::UnsupportedMethod(String::from("OPTION is not supported"));
        let actual_response = error.into_response();
        let expected_response = StatusCode::INTERNAL_SERVER_ERROR.into_response();

        assert_eq!(actual_response.status(), expected_response.status());
    }

    #[test]
    fn converts_domain_http_methods_into_http_methods() {
        assert_eq!(
            ForwardServiceRequestHttpMethod::try_from(&Method::GET).unwrap(),
            ForwardServiceRequestHttpMethod::Get
        );

        assert_eq!(
            ForwardServiceRequestHttpMethod::try_from(&Method::POST).unwrap(),
            ForwardServiceRequestHttpMethod::Post
        );

        assert_eq!(
            ForwardServiceRequestHttpMethod::try_from(&Method::PUT).unwrap(),
            ForwardServiceRequestHttpMethod::Put
        );

        assert_eq!(
            ForwardServiceRequestHttpMethod::try_from(&Method::DELETE).unwrap(),
            ForwardServiceRequestHttpMethod::Delete
        );

        assert_eq!(
            ForwardServiceRequestHttpMethod::try_from(&Method::PATCH).unwrap(),
            ForwardServiceRequestHttpMethod::Patch
        );

        let err = ForwardServiceRequestHttpMethod::try_from(&Method::OPTIONS).unwrap_err();
        match err {
            ForwardServiceRequestError::UnsupportedMethod(m) => {
                assert_eq!(m, "OPTIONS".to_string())
            }
        }
    }

    #[test]
    fn converts_http_methods_into_domain_http_methods() {
        assert_eq!(
            Method::from(ForwardServiceRequestHttpMethod::Get),
            Method::GET
        );
        assert_eq!(
            Method::from(ForwardServiceRequestHttpMethod::Post),
            Method::POST
        );
        assert_eq!(
            Method::from(ForwardServiceRequestHttpMethod::Put),
            Method::PUT
        );
        assert_eq!(
            Method::from(ForwardServiceRequestHttpMethod::Delete),
            Method::DELETE
        );
        assert_eq!(
            Method::from(ForwardServiceRequestHttpMethod::Patch),
            Method::PATCH
        );
    }
}
