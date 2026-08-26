use std::path::Path;

use crate::error::StorageError;

/// Storage-level operations LayerFS needs regardless of the underlying
/// filesystem. Correctness (crash safety, atomic activation) must not
/// depend on any single implementation's specific behavior.
pub trait StorageBackend {
    /// Creates a new writable layer at `dest`, empty or cloned from `source`.
    fn prepare_layer(&self, dest: &Path, source: Option<&Path>) -> Result<(), StorageError>;

    /// Clones an existing layer into a new independent one.
    fn clone_layer(&self, source: &Path, dest: &Path) -> Result<(), StorageError>;

    /// Makes a staged layer read-only and eligible for activation.
    fn freeze_layer(&self, layer: &Path) -> Result<(), StorageError>;

    /// Removes a layer no longer referenced by active or staged state.
    fn delete_layer(&self, layer: &Path) -> Result<(), StorageError>;

    /// Atomically swaps the active state pointer to the given layer set.
    fn activate_state(&self, layers: &[&Path]) -> Result<(), StorageError>;

    /// Performs a structural integrity check of a layer.
    fn verify_layer(&self, layer: &Path) -> Result<(), StorageError>;
}
