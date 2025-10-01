use std::{
    collections::HashMap,
    fmt::{self, Display},
    ops::{Deref, DerefMut},
};

use bytes::Bytes;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WakandaHttpServiceHeaders(pub HashMap<String, String>);

impl WakandaHttpServiceHeaders {
    pub fn get(&self, key: &str) -> Option<&String> {
        HashMap::get(self, key)
    }
}

impl Deref for WakandaHttpServiceHeaders {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WakandaHttpServiceHeaders {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone)]
pub struct WakandaHttpServiceRequest {
    pub method: WakandaHttpServiceRequestHttpMethod,
    pub url: String,
    pub headers: WakandaHttpServiceHeaders,
    pub body: Bytes,
}

impl<const N: usize> From<[(String, String); N]> for WakandaHttpServiceHeaders {
    fn from(arr: [(String, String); N]) -> Self {
        let map = arr.into_iter().collect();
        WakandaHttpServiceHeaders(map)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WakandaHttpServiceRequestHttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl Display for WakandaHttpServiceRequestHttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            WakandaHttpServiceRequestHttpMethod::Get => "GET",
            WakandaHttpServiceRequestHttpMethod::Post => "POST",
            WakandaHttpServiceRequestHttpMethod::Put => "PUT",
            WakandaHttpServiceRequestHttpMethod::Delete => "DELETE",
            WakandaHttpServiceRequestHttpMethod::Patch => "PATCH",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WakandaHttpServiceRequestError {
    #[error("HTTP method {0} is not supported")]
    UnsupportedMethod(String),
}

#[cfg(test)]
mod tests {
    use crate::wakanda_http_service::wakanda_http_service_request::WakandaHttpServiceRequestHttpMethod;

    #[test]
    fn wakanda_http_service_request_http_method_to_string() {
        let methods = [
            WakandaHttpServiceRequestHttpMethod::Get,
            WakandaHttpServiceRequestHttpMethod::Post,
            WakandaHttpServiceRequestHttpMethod::Put,
            WakandaHttpServiceRequestHttpMethod::Delete,
            WakandaHttpServiceRequestHttpMethod::Patch,
        ];

        let expected = ["GET", "POST", "PUT", "DELETE", "PATCH"];

        for (method, &expected_str) in methods.iter().zip(expected.iter()) {
            assert_eq!(method.to_string(), expected_str);
        }
    }
}
