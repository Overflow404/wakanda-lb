use std::{
    collections::HashMap,
    fmt::{self, Display},
    ops::{Deref, DerefMut},
};

use bytes::Bytes;

#[derive(Debug, Clone, Default)]
pub struct ForwardServiceRequestHeaders(pub HashMap<String, String>);

impl ForwardServiceRequestHeaders {
    pub fn get(&self, key: &str) -> Option<&String> {
        HashMap::get(self, key)
    }
}

impl Deref for ForwardServiceRequestHeaders {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ForwardServiceRequestHeaders {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone)]
pub struct ForwardServiceRequest {
    pub method: ForwardServiceRequestHttpMethod,
    pub url: String,
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
