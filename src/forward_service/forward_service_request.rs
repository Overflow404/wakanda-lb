use std::{
    collections::HashMap,
    fmt::{self, Display},
};

use bytes::Bytes;

#[derive(Debug, Clone, Default)]
pub struct ForwardServiceRequestHeaders(pub HashMap<String, String>);

impl ForwardServiceRequestHeaders {
    pub fn get(&self, key: &str) -> Option<&String> {
        self.0.get(key)
    }
}

#[derive(Debug, Clone)]
pub struct ForwardServiceRequest {
    pub method: ForwardServiceRequestHttpMethod,
    pub path: String,
    pub headers: ForwardServiceRequestHeaders,
    pub body: Bytes,
}

impl<const N: usize> From<[(String, String); N]> for ForwardServiceRequestHeaders {
    fn from(arr: [(String, String); N]) -> Self {
        let map = arr.into_iter().collect();
        ForwardServiceRequestHeaders(map)
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
