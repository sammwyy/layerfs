//! Storage backend abstraction: how layers are cloned, frozen, and
//! activated on disk, independent of the OverlayFS assembly logic.

mod backend;
pub mod boot;
mod btrfs;
mod copy_tree;
mod directory;
mod discover;
mod error;
pub mod generations;
pub mod live_update;
mod opaque;
pub mod overlay;
pub mod risk;
pub mod squash;
pub mod validate;
mod whiteout;

pub use backend::StorageBackend;
pub use btrfs::BtrfsBackend;
pub use directory::DirectoryBackend;
pub use discover::{DiscoveredStore, discover};
pub use error::StorageError;
pub use whiteout::is_whiteout;
