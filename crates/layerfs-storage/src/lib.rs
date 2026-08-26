//! Storage backend abstraction: how layers are cloned, frozen, and
//! activated on disk, independent of the OverlayFS assembly logic.

mod backend;
mod btrfs;
mod directory;
mod error;

pub use backend::StorageBackend;
pub use btrfs::BtrfsBackend;
pub use directory::DirectoryBackend;
pub use error::StorageError;
