use async_trait::async_trait;
use axum::response::IntoResponse;
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use tracing::info;

use crate::http_client::{
    http_client::HttpClient,
    error::{Error, HttpClientErrorChecker},
    request::{
        RequestHeaders, Request, RequestMethod, HttpClientRequestRequestError
    },
    response::Response,
};

#[derive(Clone)]
pub struct ReqwestHttpClient {
    client: reqwest::Client,
}

impl ReqwestHttpClient {
    #[allow(dead_code)]
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for ReqwestHttpClient {
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
impl HttpClient for ReqwestHttpClient {
    async fn execute(
        &self,
        request: Request,
    ) -> Result<Response, Error> {
        info!("Proxying {:#?}", request);

        let reqwuest_builder = self
            .client
            .request(request.method.into(), request.url)
            .headers(request.headers.into())
            .body(request.body);

        let reqwest_response = reqwuest_builder
            .send()
            .await
            .map_err(Error::from)?;

        let http_status = reqwest_response.status().as_u16();

        let headers: RequestHeaders = reqwest_response.headers().into();

        let body = reqwest_response
            .bytes()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        Ok(Response {
            status: http_status,
            headers,
            body,
        })
    }
}

impl HttpClientErrorChecker for reqwest::Error {
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

impl<T: HttpClientErrorChecker> From<T> for Error {
    fn from(err: T) -> Self {
        if err.is_timeout() {
            Error::Timeout
        } else if err.is_connect() || err.is_request() {
            Error::Network(err.error_string())
        } else {
            Error::InvalidRequest(err.error_string())
        }
    }
}

impl From<&HeaderMap> for RequestHeaders {
    fn from(headers: &HeaderMap) -> Self {
        let map = headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
            .collect();
        RequestHeaders(map)
    }
}

impl From<HeaderMap> for RequestHeaders {
    fn from(headers: HeaderMap) -> Self {
        let map = headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
            .collect();
        RequestHeaders(map)
    }
}

impl From<RequestHeaders> for HeaderMap {
    fn from(h: RequestHeaders) -> Self {
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

impl IntoResponse for HttpClientRequestRequestError {
    fn into_response(self) -> axum::response::Response {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

impl TryFrom<&Method> for RequestMethod {
    type Error = HttpClientRequestRequestError;

    fn try_from(value: &Method) -> Result<Self, Self::Error> {
        match *value {
            Method::GET => Ok(RequestMethod::Get),
            Method::POST => Ok(RequestMethod::Post),
            Method::PUT => Ok(RequestMethod::Put),
            Method::DELETE => Ok(RequestMethod::Delete),
            Method::PATCH => Ok(RequestMethod::Patch),
            _ => Err(HttpClientRequestRequestError::UnsupportedMethod(
                value.to_string(),
            )),
        }
    }
}

impl From<RequestMethod> for reqwest::Method {
    fn from(value: RequestMethod) -> Self {
        match value {
            RequestMethod::Get => reqwest::Method::GET,
            RequestMethod::Post => reqwest::Method::POST,
            RequestMethod::Put => reqwest::Method::PUT,
            RequestMethod::Delete => reqwest::Method::DELETE,
            RequestMethod::Patch => reqwest::Method::PATCH,
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;
    use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};

    use crate::http_client::{
        error::{Error, MockHttpClientErrorChecker},
        request::{
            RequestHeaders, RequestMethod, HttpClientRequestRequestError
        },
    };

    #[test]
    fn converts_reqwest_errors_into_domain_variants() {
        let mut mock = MockHttpClientErrorChecker::new();
        mock.expect_is_timeout().return_const(true);
        let result: Error = mock.into();
        assert!(matches!(result, Error::Timeout));

        mock = MockHttpClientErrorChecker::new();
        mock.expect_is_timeout().return_const(false);
        mock.expect_is_connect().return_const(true);
        mock.expect_error_string()
            .return_const("connect error".to_string());
        let result: Error = mock.into();
        assert!(matches!(result, Error::Network(_)));

        mock = MockHttpClientErrorChecker::new();
        mock.expect_is_timeout().return_const(false);
        mock.expect_is_connect().return_const(false);
        mock.expect_is_request().return_const(true);
        mock.expect_error_string()
            .return_const("request error".to_string());
        let result: Error = mock.into();
        assert!(matches!(result, Error::Network(_)));

        mock = MockHttpClientErrorChecker::new();
        mock.expect_is_timeout().return_const(false);
        mock.expect_is_connect().return_const(false);
        mock.expect_is_request().return_const(false);
        mock.expect_error_string()
            .return_const("other error".to_string());
        let result: Error = mock.into();
        assert!(matches!(result, Error::InvalidRequest(_)));
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

        let result_borrowed: RequestHeaders = (&headers).into();
        let result_owned: RequestHeaders = headers.into();

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
        let mut http_client_request_headers = RequestHeaders::default();
        http_client_request_headers
            .insert("content-type".to_string(), "application/json".to_string());
        http_client_request_headers
            .insert("x-custom-header".to_string(), "custom-value".to_string());

        let result: HeaderMap = http_client_request_headers.into();
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
        let error = HttpClientRequestRequestError::UnsupportedMethod(String::from(
            "OPTION is not supported",
        ));
        let actual_response = error.into_response();
        let expected_response = StatusCode::INTERNAL_SERVER_ERROR.into_response();

        assert_eq!(actual_response.status(), expected_response.status());
    }

    #[test]
    fn converts_domain_http_methods_into_http_methods() {
        assert_eq!(
            RequestMethod::try_from(&Method::GET).unwrap(),
            RequestMethod::Get
        );

        assert_eq!(
            RequestMethod::try_from(&Method::POST).unwrap(),
            RequestMethod::Post
        );

        assert_eq!(
            RequestMethod::try_from(&Method::PUT).unwrap(),
            RequestMethod::Put
        );

        assert_eq!(
            RequestMethod::try_from(&Method::DELETE).unwrap(),
            RequestMethod::Delete
        );

        assert_eq!(
            RequestMethod::try_from(&Method::PATCH).unwrap(),
            RequestMethod::Patch
        );

        let err = RequestMethod::try_from(&Method::OPTIONS).unwrap_err();
        match err {
            HttpClientRequestRequestError::UnsupportedMethod(m) => {
                assert_eq!(m, "OPTIONS".to_string())
            }
        }
    }

    #[test]
    fn converts_http_methods_into_domain_http_methods() {
        assert_eq!(
            Method::from(RequestMethod::Get),
            Method::GET
        );
        assert_eq!(
            Method::from(RequestMethod::Post),
            Method::POST
        );
        assert_eq!(
            Method::from(RequestMethod::Put),
            Method::PUT
        );
        assert_eq!(
            Method::from(RequestMethod::Delete),
            Method::DELETE
        );
        assert_eq!(
            Method::from(RequestMethod::Patch),
            Method::PATCH
        );
    }
}
