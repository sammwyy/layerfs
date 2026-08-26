use std::path::{Path, PathBuf};

use crate::error::StorageError;

/// Result of scanning a `DirectoryBackend` store root for the layers it
/// contains. Purely a filesystem read (`exists()` checks); no mounting.
#[derive(Debug, Clone)]
pub struct DiscoveredStore {
    pub base: PathBuf,
    pub update: Option<PathBuf>,
    pub update_head: Option<PathBuf>,
    pub r#override: Option<PathBuf>,
    pub data: Vec<PathBuf>,
    /// Workdir for OverlayFS, required alongside any upperdir. Not part of
    /// the layout in the design notes; added because overlay mounts need
    /// it and it must live on the same filesystem as `override`.
    pub work: PathBuf,
}

/// Scans `root` for the fixed `base/update/update-head/override/data/work`
/// layout described for the generic directory backend.
pub fn discover(root: &Path) -> Result<DiscoveredStore, StorageError> {
    let present = |name: &str| -> Option<PathBuf> {
        let path = root.join(name);
        path.is_dir().then_some(path)
    };

    let base = present("base").ok_or_else(|| {
        StorageError::Discovery(format!("no base layer found under {}", root.display()))
    })?;

    let data = present("data").map(|dir| vec![dir]).unwrap_or_default();

    Ok(DiscoveredStore {
        base,
        update: present("update"),
        update_head: present("update-head"),
        r#override: present("override"),
        data,
        work: root.join("work"),
    })
}
