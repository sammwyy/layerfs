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

    #[error("inconsistent store state: {0}")]
    InconsistentState(String),

    #[error("validation failed: {0}")]
    ValidationFailed(String),

    #[error(transparent)]
    Storage(#[from] layerfs_storage::StorageError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
