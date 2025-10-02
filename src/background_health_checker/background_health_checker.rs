use async_trait::async_trait;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait BackgroundChecker: Send + Sync {
    async fn execute(&self);
}
