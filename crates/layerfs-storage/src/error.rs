use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("{0} is not implemented yet")]
    NotImplemented(&'static str),

    #[error("backend does not support this operation: {0}")]
    Unsupported(&'static str),

    #[error("layer discovery failed: {0}")]
    Discovery(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
