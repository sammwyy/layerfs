use std::path::Path;

use crate::backend::StorageBackend;
use crate::error::StorageError;

/// Backend using Btrfs subvolumes and CoW snapshots for cheap cloning and
/// atomic-ish freeze/activate operations. First-class MVP target.
pub struct BtrfsBackend {
    /// Path to the `layerfs` subvolume root (e.g. `/mnt/btrfs-top/layerfs`).
    pub root: std::path::PathBuf,
}

impl BtrfsBackend {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl StorageBackend for BtrfsBackend {
    fn prepare_layer(&self, _dest: &Path, _source: Option<&Path>) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented("BtrfsBackend::prepare_layer"))
    }

    fn clone_layer(&self, _source: &Path, _dest: &Path) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented("BtrfsBackend::clone_layer"))
    }

    fn freeze_layer(&self, _layer: &Path) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented("BtrfsBackend::freeze_layer"))
    }

    fn delete_layer(&self, _layer: &Path) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented("BtrfsBackend::delete_layer"))
    }

    fn activate_state(&self, _layers: &[&Path]) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented("BtrfsBackend::activate_state"))
    }

    fn verify_layer(&self, _layer: &Path) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented("BtrfsBackend::verify_layer"))
    }
}
