//! Pure LayerFS semantics: checkpoints, layers, and state metadata. No I/O
//! beyond what `state` requires; layer squashing lives in
//! `layerfs-storage::squash` instead, to avoid a circular dependency.

mod boot_options;
mod checkpoint;
mod error;
mod layer;
mod path_class;
mod state;

pub use boot_options::BootOptions;
pub use checkpoint::Checkpoint;
pub use error::CoreError;
pub use layer::{Layer, LayerKind, LayerStack};
pub use path_class::{DATA_MOUNTS, PathClass, classify};
pub use state::{State, UpdateState};
