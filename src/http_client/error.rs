#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Timeout")]
    Timeout,
}

#[cfg_attr(test, mockall::automock)]
pub trait HttpClientErrorChecker {
    fn is_timeout(&self) -> bool;
    fn is_connect(&self) -> bool;
    fn is_request(&self) -> bool;
    fn error_string(&self) -> String;
}
