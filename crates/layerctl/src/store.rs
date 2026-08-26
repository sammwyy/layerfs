use std::path::{Path, PathBuf};

use layerfs_storage::{DiscoveredStore, discover};

/// Default store root when `--store` is not given. Matches the transient
/// mount point section 19 of the design notes uses during boot; on a
/// running system this is normally where the backing store is exposed for
/// administration.
const DEFAULT_STORE: &str = "/run/layerfs-store";

pub fn resolve(store: &Option<PathBuf>) -> PathBuf {
    store
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_STORE))
}

pub fn discover_layers(store: &Path) -> Result<DiscoveredStore, String> {
    discover(store).map_err(|e| format!("{e} (pass --store <path> to point at one)"))
}

/// Resolves a layer name from CLI arguments to its discovered path.
pub fn layer_path(name: &str, discovered: &DiscoveredStore) -> Result<PathBuf, String> {
    match name {
        "base" => Ok(discovered.base.clone()),
        "update" => discovered
            .update
            .clone()
            .ok_or_else(|| "no update layer present".to_string()),
        "update-head" => discovered
            .update_head
            .clone()
            .ok_or_else(|| "no update-head layer present".to_string()),
        "override" => discovered
            .r#override
            .clone()
            .ok_or_else(|| "no override layer present".to_string()),
        "data" => discovered
            .data
            .clone()
            .ok_or_else(|| "no data store present".to_string()),
        other => Err(format!(
            "unknown layer '{other}' (expected base, update, update-head, override, or data)"
        )),
    }
}

/// The layer directly below `name` in the layer stack, used to classify
/// diff entries as added vs. modified. `None` means there is nothing
/// beneath it to compare against (base has no lower layer).
pub fn layer_below(name: &str, discovered: &DiscoveredStore) -> Option<PathBuf> {
    match name {
        "override" => discovered
            .update_head
            .clone()
            .or_else(|| discovered.update.clone())
            .or_else(|| Some(discovered.base.clone())),
        "update-head" => discovered
            .update
            .clone()
            .or_else(|| Some(discovered.base.clone())),
        "update" => Some(discovered.base.clone()),
        _ => None,
    }
}
