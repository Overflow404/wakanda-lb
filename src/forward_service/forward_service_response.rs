use std::collections::HashMap;

use bytes::Bytes;

#[derive(Debug, Clone)]
pub struct ForwardServiceResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
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

    #[error("Server error: {status}")]
    ServerError { status: u16 },
}
