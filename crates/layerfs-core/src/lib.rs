//! Pure LayerFS semantics: checkpoints, layers, state metadata, and layer
//! squashing. No I/O beyond what `state` and `squash` explicitly require;
//! consumers (layerfs-init, layerctl) own mounting and process execution.

mod boot_options;
mod checkpoint;
mod error;
mod layer;
mod path_class;
mod squash;
mod state;

pub use boot_options::BootOptions;
pub use checkpoint::Checkpoint;
pub use error::CoreError;
pub use layer::{Layer, LayerKind, LayerStack};
pub use path_class::{PathClass, classify};
pub use squash::squash;
pub use state::{State, UpdateState};
