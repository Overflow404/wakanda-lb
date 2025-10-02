use crate::select_server::{error::Error, request::Request, response::Response};

#[cfg_attr(test, mockall::automock)]
pub(crate) trait SelectServer: Send + Sync {
    fn execute(&self, request: Request) -> Result<Response, Error>;
}
