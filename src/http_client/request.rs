use std::{
    collections::HashMap,
    fmt::{self, Display},
    ops::{Deref, DerefMut},
};

use bytes::Bytes;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: RequestMethod,
    pub url: String,
    pub headers: RequestHeaders,
    pub body: Bytes,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RequestHeaders(pub HashMap<String, String>);

impl Deref for RequestHeaders {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RequestHeaders {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const N: usize> From<[(String, String); N]> for RequestHeaders {
    fn from(arr: [(String, String); N]) -> Self {
        let map = arr.into_iter().collect();
        RequestHeaders(map)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequestMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl Display for RequestMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            RequestMethod::Get => "GET",
            RequestMethod::Post => "POST",
            RequestMethod::Put => "PUT",
            RequestMethod::Delete => "DELETE",
            RequestMethod::Patch => "PATCH",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("HTTP method {0} is not supported")]
    UnsupportedMethod(String),
}

#[cfg(test)]
mod tests {
    use crate::http_client::request::RequestMethod;

    #[test]
    fn http_client_request_http_method_to_string() {
        let methods = [
            RequestMethod::Get,
            RequestMethod::Post,
            RequestMethod::Put,
            RequestMethod::Delete,
            RequestMethod::Patch,
        ];

        let expected = ["GET", "POST", "PUT", "DELETE", "PATCH"];

        for (method, &expected_str) in methods.iter().zip(expected.iter()) {
            assert_eq!(method.to_string(), expected_str);
        }
    }
}
