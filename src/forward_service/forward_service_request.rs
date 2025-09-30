use std::{
    collections::HashMap,
    fmt::{self, Display},
};

use bytes::Bytes;
use http::Method;

#[derive(Debug, Clone)]
pub struct ForwardServiceRequest {
    pub method: ForwardServiceRequestHttpMethod,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Bytes,
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
