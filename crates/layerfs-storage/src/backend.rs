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

const BTRFS_SUPER_MAGIC: u32 = 0x9123_683e;

/// Picks `BtrfsBackend` if `root` (or its nearest existing ancestor) sits
/// on a Btrfs filesystem, else falls back to `DirectoryBackend`.
pub fn detect_backend(root: &Path) -> Box<dyn StorageBackend> {
    let mut probe = root;
    while !probe.exists() {
        match probe.parent() {
            Some(parent) => probe = parent,
            None => break,
        }
    }

    let is_btrfs = rustix::fs::statfs(probe)
        .map(|s| s.f_type as u32 == BTRFS_SUPER_MAGIC)
        .unwrap_or(false);

    if is_btrfs {
        Box::new(crate::BtrfsBackend::new(root))
    } else {
        Box::new(crate::DirectoryBackend::new(root))
    }
}
