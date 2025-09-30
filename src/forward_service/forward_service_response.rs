use bytes::Bytes;

use crate::forward_service::forward_service_request::ForwardServiceRequestHeaders;

#[derive(Debug, Clone)]
pub struct ForwardServiceResponse {
    pub status: u16,
    pub headers: ForwardServiceRequestHeaders,
    pub body: Bytes,
}

#[derive(Debug, thiserror::Error)]
pub enum ForwardServiceError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Timeout")]
    Timeout,
}
