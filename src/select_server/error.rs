#[derive(Debug, PartialEq, thiserror::Error)]
pub(crate) enum Error {
    #[error("There are zero healthy target servers")]
    NoOneIsAlive,
    #[error("Poisoned read")]
    PoisonedRead,
}
