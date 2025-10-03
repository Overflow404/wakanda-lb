use axum::extract::Request;
use http::HeaderName;
use tower_http::request_id::{MakeRequestId, RequestId};
use uuid::Uuid;

pub const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
pub const UNKNOWN_REQUEST_ID: &str = "unknown";

#[derive(Clone, Default)]
pub struct LoadBalancerRequestId {}

impl MakeRequestId for LoadBalancerRequestId {
    fn make_request_id<B>(&mut self, _: &Request<B>) -> Option<RequestId> {
        let request_id = Uuid::new_v4().to_string().parse().unwrap();

        Some(RequestId::new(request_id))
    }
}
