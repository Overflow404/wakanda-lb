use http::{HeaderName, Request};
use uuid::Uuid;

use tower_http::request_id::{MakeRequestId, RequestId};

pub(crate) const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
pub(crate) const UNKNOWN_REQUEST_ID: &str = "unknown";

#[derive(Clone, Default)]
pub(crate) struct AlphaRequestId {}

impl MakeRequestId for AlphaRequestId {
    fn make_request_id<B>(&mut self, _: &Request<B>) -> Option<RequestId> {
        let request_id = Uuid::new_v4().to_string().parse().unwrap();

        Some(RequestId::new(request_id))
    }
}
