use async_trait::async_trait;
use axum::response::IntoResponse;
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use tracing::info;

use crate::wakanda_http_service::{
    wakanda_http_service::WakandaHttpService,
    wakanda_http_service_error::WakandaHttpServiceErrorChecker,
    wakanda_http_service_request::{
        WakandaHttpServiceHeaders, WakandaHttpServiceRequest, WakandaHttpServiceRequestError,
        WakandaHttpServiceRequestHttpMethod,
    },
    wakanda_http_service_response::{WakandaHttpServiceError, WakandaHttpServiceResponse},
};

#[derive(Clone)]
pub struct ReqwestHttpService {
    client: reqwest::Client,
}

impl ReqwestHttpService {
    #[allow(dead_code)]
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for ReqwestHttpService {
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
impl WakandaHttpService for ReqwestHttpService {
    async fn execute(
        &self,
        request: WakandaHttpServiceRequest,
    ) -> Result<WakandaHttpServiceResponse, WakandaHttpServiceError> {
        info!("Proxying {:#?}", request);

        let reqwuest_builder = self
            .client
            .request(request.method.into(), request.url)
            .headers(request.headers.into())
            .body(request.body);

        let reqwest_response = reqwuest_builder
            .send()
            .await
            .map_err(WakandaHttpServiceError::from)?;

        let http_status = reqwest_response.status().as_u16();

        let headers: WakandaHttpServiceHeaders = reqwest_response.headers().into();

        let body = reqwest_response
            .bytes()
            .await
            .map_err(|e| WakandaHttpServiceError::Network(e.to_string()))?;

        Ok(WakandaHttpServiceResponse {
            status: http_status,
            headers,
            body,
        })
    }
}

impl WakandaHttpServiceErrorChecker for reqwest::Error {
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

impl<T: WakandaHttpServiceErrorChecker> From<T> for WakandaHttpServiceError {
    fn from(err: T) -> Self {
        if err.is_timeout() {
            WakandaHttpServiceError::Timeout
        } else if err.is_connect() || err.is_request() {
            WakandaHttpServiceError::Network(err.error_string())
        } else {
            WakandaHttpServiceError::InvalidRequest(err.error_string())
        }
    }
}

impl From<&HeaderMap> for WakandaHttpServiceHeaders {
    fn from(headers: &HeaderMap) -> Self {
        let map = headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
            .collect();
        WakandaHttpServiceHeaders(map)
    }
}

impl From<HeaderMap> for WakandaHttpServiceHeaders {
    fn from(headers: HeaderMap) -> Self {
        let map = headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
            .collect();
        WakandaHttpServiceHeaders(map)
    }
}

impl From<WakandaHttpServiceHeaders> for HeaderMap {
    fn from(h: WakandaHttpServiceHeaders) -> Self {
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

impl IntoResponse for WakandaHttpServiceRequestError {
    fn into_response(self) -> axum::response::Response {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

impl TryFrom<&Method> for WakandaHttpServiceRequestHttpMethod {
    type Error = WakandaHttpServiceRequestError;

    fn try_from(value: &Method) -> Result<Self, Self::Error> {
        match *value {
            Method::GET => Ok(WakandaHttpServiceRequestHttpMethod::Get),
            Method::POST => Ok(WakandaHttpServiceRequestHttpMethod::Post),
            Method::PUT => Ok(WakandaHttpServiceRequestHttpMethod::Put),
            Method::DELETE => Ok(WakandaHttpServiceRequestHttpMethod::Delete),
            Method::PATCH => Ok(WakandaHttpServiceRequestHttpMethod::Patch),
            _ => Err(WakandaHttpServiceRequestError::UnsupportedMethod(
                value.to_string(),
            )),
        }
    }
}

impl From<WakandaHttpServiceRequestHttpMethod> for reqwest::Method {
    fn from(value: WakandaHttpServiceRequestHttpMethod) -> Self {
        match value {
            WakandaHttpServiceRequestHttpMethod::Get => reqwest::Method::GET,
            WakandaHttpServiceRequestHttpMethod::Post => reqwest::Method::POST,
            WakandaHttpServiceRequestHttpMethod::Put => reqwest::Method::PUT,
            WakandaHttpServiceRequestHttpMethod::Delete => reqwest::Method::DELETE,
            WakandaHttpServiceRequestHttpMethod::Patch => reqwest::Method::PATCH,
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;
    use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};

    use crate::wakanda_http_service::{
        wakanda_http_service_error::MockWakandaHttpServiceErrorChecker,
        wakanda_http_service_request::{
            WakandaHttpServiceHeaders, WakandaHttpServiceRequestError,
            WakandaHttpServiceRequestHttpMethod,
        },
        wakanda_http_service_response::WakandaHttpServiceError,
    };

    #[test]
    fn converts_reqwest_errors_into_domain_variants() {
        let mut mock = MockWakandaHttpServiceErrorChecker::new();
        mock.expect_is_timeout().return_const(true);
        let result: WakandaHttpServiceError = mock.into();
        assert!(matches!(result, WakandaHttpServiceError::Timeout));

        mock = MockWakandaHttpServiceErrorChecker::new();
        mock.expect_is_timeout().return_const(false);
        mock.expect_is_connect().return_const(true);
        mock.expect_error_string()
            .return_const("connect error".to_string());
        let result: WakandaHttpServiceError = mock.into();
        assert!(matches!(result, WakandaHttpServiceError::Network(_)));

        mock = MockWakandaHttpServiceErrorChecker::new();
        mock.expect_is_timeout().return_const(false);
        mock.expect_is_connect().return_const(false);
        mock.expect_is_request().return_const(true);
        mock.expect_error_string()
            .return_const("request error".to_string());
        let result: WakandaHttpServiceError = mock.into();
        assert!(matches!(result, WakandaHttpServiceError::Network(_)));

        mock = MockWakandaHttpServiceErrorChecker::new();
        mock.expect_is_timeout().return_const(false);
        mock.expect_is_connect().return_const(false);
        mock.expect_is_request().return_const(false);
        mock.expect_error_string()
            .return_const("other error".to_string());
        let result: WakandaHttpServiceError = mock.into();
        assert!(matches!(result, WakandaHttpServiceError::InvalidRequest(_)));
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

        let result_borrowed: WakandaHttpServiceHeaders = (&headers).into();
        let result_owned: WakandaHttpServiceHeaders = headers.into();

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
        let mut wakanda_http_service_request_headers = WakandaHttpServiceHeaders::default();
        wakanda_http_service_request_headers
            .insert("content-type".to_string(), "application/json".to_string());
        wakanda_http_service_request_headers
            .insert("x-custom-header".to_string(), "custom-value".to_string());

        let result: HeaderMap = wakanda_http_service_request_headers.into();
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
        let error = WakandaHttpServiceRequestError::UnsupportedMethod(String::from(
            "OPTION is not supported",
        ));
        let actual_response = error.into_response();
        let expected_response = StatusCode::INTERNAL_SERVER_ERROR.into_response();

        assert_eq!(actual_response.status(), expected_response.status());
    }

    #[test]
    fn converts_domain_http_methods_into_http_methods() {
        assert_eq!(
            WakandaHttpServiceRequestHttpMethod::try_from(&Method::GET).unwrap(),
            WakandaHttpServiceRequestHttpMethod::Get
        );

        assert_eq!(
            WakandaHttpServiceRequestHttpMethod::try_from(&Method::POST).unwrap(),
            WakandaHttpServiceRequestHttpMethod::Post
        );

        assert_eq!(
            WakandaHttpServiceRequestHttpMethod::try_from(&Method::PUT).unwrap(),
            WakandaHttpServiceRequestHttpMethod::Put
        );

        assert_eq!(
            WakandaHttpServiceRequestHttpMethod::try_from(&Method::DELETE).unwrap(),
            WakandaHttpServiceRequestHttpMethod::Delete
        );

        assert_eq!(
            WakandaHttpServiceRequestHttpMethod::try_from(&Method::PATCH).unwrap(),
            WakandaHttpServiceRequestHttpMethod::Patch
        );

        let err = WakandaHttpServiceRequestHttpMethod::try_from(&Method::OPTIONS).unwrap_err();
        match err {
            WakandaHttpServiceRequestError::UnsupportedMethod(m) => {
                assert_eq!(m, "OPTIONS".to_string())
            }
        }
    }

    #[test]
    fn converts_http_methods_into_domain_http_methods() {
        assert_eq!(
            Method::from(WakandaHttpServiceRequestHttpMethod::Get),
            Method::GET
        );
        assert_eq!(
            Method::from(WakandaHttpServiceRequestHttpMethod::Post),
            Method::POST
        );
        assert_eq!(
            Method::from(WakandaHttpServiceRequestHttpMethod::Put),
            Method::PUT
        );
        assert_eq!(
            Method::from(WakandaHttpServiceRequestHttpMethod::Delete),
            Method::DELETE
        );
        assert_eq!(
            Method::from(WakandaHttpServiceRequestHttpMethod::Patch),
            Method::PATCH
        );
    }
}
