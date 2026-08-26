use std::path::PathBuf;

use layerfs_core::UpdateState;
use layerfs_storage::StorageBackend;

use crate::error::TransactionError;
use crate::lock::TransactionLock;
use crate::state::{TransactionRecord, TransactionState};

/// Drives one system transaction: staging → validation → atomic commit.
///
/// Never mutates the active UPDATE/UPDATE_HEAD in place. A crash before
/// `commit()` returns must leave the previously active state untouched.
pub struct Transaction<'a> {
    _lock: TransactionLock,
    backend: &'a dyn StorageBackend,
    record: TransactionRecord,
}

impl<'a> Transaction<'a> {
    pub fn begin(
        lock_path: &std::path::Path,
        backend: &'a dyn StorageBackend,
        id: impl Into<String>,
        adapter: impl Into<String>,
    ) -> Result<Self, TransactionError> {
        let lock = TransactionLock::acquire(lock_path)?;
        let record = TransactionRecord {
            id: id.into(),
            kind: "system".to_string(),
            started_at_unix: 0,
            adapter: adapter.into(),
            state: TransactionState::Preparing,
        };

        Ok(Self {
            _lock: lock,
            backend,
            record,
        })
    }

    /// Clones the active UPDATE, squashes the active UPDATE_HEAD into it,
    /// and prepares an empty writable HEAD.next for the transaction body.
    pub fn stage(
        &mut self,
        _active: &UpdateState,
        _staging_dir: &PathBuf,
    ) -> Result<(), TransactionError> {
        self.record.state = TransactionState::Running;
        Err(TransactionError::NotImplemented("Transaction::stage"))
    }

    /// Runs MVP structural checks against the staged root.
    pub fn validate(&mut self, _staging_dir: &PathBuf) -> Result<(), TransactionError> {
        self.record.state = TransactionState::Validating;
        Err(TransactionError::NotImplemented("Transaction::validate"))
    }

    /// Freezes the staged layers and atomically activates them as the new
    /// UPDATE/UPDATE_HEAD pair. Only this step may change active state.
    pub fn commit(&mut self, _next: &UpdateState) -> Result<(), TransactionError> {
        self.record.state = TransactionState::Committing;
        let _ = self.backend;
        Err(TransactionError::NotImplemented("Transaction::commit"))
    }
}
