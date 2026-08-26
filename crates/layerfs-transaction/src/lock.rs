use std::fs::{File, OpenOptions};
use std::path::Path;

use rustix::fs::{FlockOperation, flock};

use crate::error::TransactionError;

/// Exclusive advisory lock ensuring only one system transaction runs at a
/// time. Backed by `flock(2)` rather than PID-file existence checks, which
/// do not reliably detect a dead holder.
pub struct TransactionLock {
    _file: File,
}

impl TransactionLock {
    /// Acquires the lock at `path`, creating it if necessary. Blocks the
    /// caller instead of racing a stale PID file.
    pub fn acquire(path: &Path) -> Result<Self, TransactionError> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(TransactionError::Io)?;

        flock(&file, FlockOperation::LockExclusive)
            .map_err(|e| TransactionError::Lock(e.to_string()))?;

        Ok(Self { _file: file })
    }
}

impl Drop for TransactionLock {
    fn drop(&mut self) {
        let _ = flock(&self._file, FlockOperation::Unlock);
    }
}
