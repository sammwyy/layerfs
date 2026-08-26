use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("{0} is not implemented yet")]
    NotImplemented(&'static str),

    #[error("failed to acquire transaction lock: {0}")]
    Lock(String),

    #[error(transparent)]
    Storage(#[from] layerfs_storage::StorageError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
