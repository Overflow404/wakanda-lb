#[derive(Debug, PartialEq, thiserror::Error)]
pub enum SelectServerServiceError {
    #[error("There are zero healthy target servers")]
    NoOneIsAlive,
    #[error("Poisoned read")]
    PoisonedRead,
}
