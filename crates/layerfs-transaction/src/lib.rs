//! System transaction lifecycle. Wraps `layerfs-storage` operations with
//! locking, staging, validation, and crash-safe atomic commit as described
//! in the LayerFS transaction model.

mod error;
mod lock;
pub mod manifest;
mod state;
mod transaction;

pub use error::TransactionError;
pub use lock::TransactionLock;
pub use state::{TransactionRecord, TransactionState};
pub use transaction::Transaction;
