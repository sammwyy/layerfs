use std::path::Path;

use crate::backend::StorageBackend;
use crate::error::StorageError;

/// Fallback backend for filesystems without native snapshotting (e.g.
/// ext4). Cloning and squashing require filesystem-level copying and are
/// less efficient than `BtrfsBackend`; correctness must not depend on this
/// being fast.
pub struct DirectoryBackend {
    /// Path to `.layerfs-store`.
    pub root: std::path::PathBuf,
}

impl DirectoryBackend {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl StorageBackend for DirectoryBackend {
    fn prepare_layer(&self, _dest: &Path, _source: Option<&Path>) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented(
            "DirectoryBackend::prepare_layer",
        ))
    }

    fn clone_layer(&self, _source: &Path, _dest: &Path) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented(
            "DirectoryBackend::clone_layer",
        ))
    }

    fn freeze_layer(&self, _layer: &Path) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented(
            "DirectoryBackend::freeze_layer",
        ))
    }

    fn delete_layer(&self, _layer: &Path) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented(
            "DirectoryBackend::delete_layer",
        ))
    }

    fn activate_state(&self, _layers: &[&Path]) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented(
            "DirectoryBackend::activate_state",
        ))
    }

    fn verify_layer(&self, _layer: &Path) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented(
            "DirectoryBackend::verify_layer",
        ))
    }
}
