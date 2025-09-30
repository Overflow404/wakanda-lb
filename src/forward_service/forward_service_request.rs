use std::{
    collections::HashMap,
    fmt::{self, Display},
};

use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, Method};

#[derive(Debug, Clone, Default)]
pub struct ForwardServiceRequestHeaders(HashMap<String, String>);

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

impl From<&Method> for ForwardServiceRequestHttpMethod {
    fn from(value: &Method) -> Self {
        match *value {
            Method::GET => ForwardServiceRequestHttpMethod::Get,
            Method::POST => ForwardServiceRequestHttpMethod::Post,
            Method::PUT => ForwardServiceRequestHttpMethod::Put,
            Method::DELETE => ForwardServiceRequestHttpMethod::Delete,
            Method::PATCH => ForwardServiceRequestHttpMethod::Patch,
            _ => panic!("Not supported"),
        }
    }
}
