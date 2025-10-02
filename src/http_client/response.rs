use bytes::Bytes;

use crate::http_client::request::RequestHeaders;

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: RequestHeaders,
    pub body: Bytes,
}
