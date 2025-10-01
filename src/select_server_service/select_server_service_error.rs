#[derive(Debug, thiserror::Error)]
pub enum SelectServerServiceError {
    #[error("There are zero healthy target servers")]
    NoOneIsAlive,
}
