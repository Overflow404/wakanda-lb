#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("There are zero healthy target servers")]
    NoOneIsAlive,
    #[error("Poisoned read")]
    PoisonedRead,
}
