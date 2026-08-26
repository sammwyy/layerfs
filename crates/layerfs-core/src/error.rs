use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid checkpoint value: {0}")]
    InvalidCheckpoint(String),

    #[error("invalid value for {key}: {value}")]
    InvalidOption { key: String, value: String },

    #[error("malformed state metadata: {0}")]
    InvalidState(String),

    #[error("{0} is not implemented yet")]
    NotImplemented(&'static str),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
