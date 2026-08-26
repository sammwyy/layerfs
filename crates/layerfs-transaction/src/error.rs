use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("{0} is not implemented yet")]
    NotImplemented(&'static str),

    #[error("failed to acquire transaction lock: {0}")]
    Lock(String),

    #[error("failed to create private mount namespace: {0}")]
    Namespace(String),

    #[error("transaction is already staged")]
    AlreadyStaged,

    #[error("transaction has not been staged yet")]
    NotStaged,

    #[error(
        "an active UPDATE_HEAD exists; consolidating it into UPDATE requires layer squashing, which is not implemented yet (milestone 6)"
    )]
    SquashRequired,

    #[error("validation failed: {0}")]
    ValidationFailed(String),

    #[error(transparent)]
    Storage(#[from] layerfs_storage::StorageError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
