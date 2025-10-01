use std::{
    collections::HashMap,
    fmt::{self, Display},
    ops::{Deref, DerefMut},
};

use bytes::Bytes;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ForwardServiceHeaders(pub HashMap<String, String>);

impl ForwardServiceHeaders {
    pub fn get(&self, key: &str) -> Option<&String> {
        HashMap::get(self, key)
    }
}

impl Deref for ForwardServiceHeaders {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ForwardServiceHeaders {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone)]
pub struct ForwardServiceRequest {
    pub method: ForwardServiceRequestHttpMethod,
    pub url: String,
    pub headers: ForwardServiceHeaders,
    pub body: Bytes,
}

impl<const N: usize> From<[(String, String); N]> for ForwardServiceHeaders {
    fn from(arr: [(String, String); N]) -> Self {
        let map = arr.into_iter().collect();
        ForwardServiceHeaders(map)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForwardServiceRequestHttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl Display for ForwardServiceRequestHttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ForwardServiceRequestHttpMethod::Get => "GET",
            ForwardServiceRequestHttpMethod::Post => "POST",
            ForwardServiceRequestHttpMethod::Put => "PUT",
            ForwardServiceRequestHttpMethod::Delete => "DELETE",
            ForwardServiceRequestHttpMethod::Patch => "PATCH",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ForwardServiceRequestError {
    #[error("HTTP method {0} is not supported")]
    UnsupportedMethod(String),
}

#[cfg(test)]
mod tests {
    use crate::forward_service::forward_service_request::ForwardServiceRequestHttpMethod;

    #[test]
    fn forward_service_request_http_method_to_string() {
        let methods = [
            ForwardServiceRequestHttpMethod::Get,
            ForwardServiceRequestHttpMethod::Post,
            ForwardServiceRequestHttpMethod::Put,
            ForwardServiceRequestHttpMethod::Delete,
            ForwardServiceRequestHttpMethod::Patch,
        ];

        let expected = ["GET", "POST", "PUT", "DELETE", "PATCH"];

        for (method, &expected_str) in methods.iter().zip(expected.iter()) {
            assert_eq!(method.to_string(), expected_str);
        }
    }
}
