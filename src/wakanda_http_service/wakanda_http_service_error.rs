#[cfg_attr(test, mockall::automock)]
pub trait WakandaHttpServiceErrorChecker {
    fn is_timeout(&self) -> bool;
    fn is_connect(&self) -> bool;
    fn is_request(&self) -> bool;
    fn error_string(&self) -> String;
}
