use bytes::Bytes;

use crate::wakanda_http_service::wakanda_http_service_request::WakandaHttpServiceHeaders;

#[derive(Debug, Clone)]
pub struct WakandaHttpServiceResponse {
    pub status: u16,
    pub headers: WakandaHttpServiceHeaders,
    pub body: Bytes,
}

#[derive(Debug, thiserror::Error)]
pub enum WakandaHttpServiceError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Timeout")]
    Timeout,
}
