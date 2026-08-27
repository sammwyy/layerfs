use std::path::Path;
use std::process::Command;

use crate::backend::StorageBackend;
use crate::copy_tree::copy_tree;
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

fn run_btrfs(args: &[&std::ffi::OsStr]) -> Result<(), StorageError> {
    let output = Command::new("btrfs").args(args).output()?;
    if !output.status.success() {
        return Err(StorageError::Discovery(format!(
            "btrfs {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

impl StorageBackend for BtrfsBackend {
    fn prepare_layer(&self, dest: &Path, source: Option<&Path>) -> Result<(), StorageError> {
        run_btrfs(&[
            std::ffi::OsStr::new("subvolume"),
            std::ffi::OsStr::new("create"),
            dest.as_os_str(),
        ])?;
        if let Some(source) = source {
            for entry in std::fs::read_dir(source)? {
                let entry = entry?;
                copy_tree(&entry.path(), &dest.join(entry.file_name()))?;
            }
        }
        Ok(())
    }

    fn clone_layer(&self, source: &Path, dest: &Path) -> Result<(), StorageError> {
        run_btrfs(&[
            std::ffi::OsStr::new("subvolume"),
            std::ffi::OsStr::new("snapshot"),
            source.as_os_str(),
            dest.as_os_str(),
        ])
    }

    fn freeze_layer(&self, layer: &Path) -> Result<(), StorageError> {
        run_btrfs(&[
            std::ffi::OsStr::new("property"),
            std::ffi::OsStr::new("set"),
            layer.as_os_str(),
            std::ffi::OsStr::new("ro"),
            std::ffi::OsStr::new("true"),
        ])
    }

    fn delete_layer(&self, layer: &Path) -> Result<(), StorageError> {
        run_btrfs(&[
            std::ffi::OsStr::new("property"),
            std::ffi::OsStr::new("set"),
            layer.as_os_str(),
            std::ffi::OsStr::new("ro"),
            std::ffi::OsStr::new("false"),
        ])?;
        run_btrfs(&[
            std::ffi::OsStr::new("subvolume"),
            std::ffi::OsStr::new("delete"),
            layer.as_os_str(),
        ])
    }

    fn activate_state(&self, layers: &[&Path]) -> Result<(), StorageError> {
        let listing = layers
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let tmp = self.root.join(".active.tmp");
        let dest = self.root.join(".active");

        std::fs::write(&tmp, listing)?;
        std::fs::rename(&tmp, &dest)?;

        Ok(())
    }

    fn verify_layer(&self, layer: &Path) -> Result<(), StorageError> {
        let output = Command::new("btrfs")
            .arg("subvolume")
            .arg("show")
            .arg(layer)
            .output()?;
        if !output.status.success() {
            return Err(StorageError::Discovery(format!(
                "{} is not a btrfs subvolume",
                layer.display()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Requires a real Btrfs mount: not available in a plain unprivileged
    /// user namespace, so these run only where `LAYERFS_BTRFS_TEST_ROOT`
    /// points at one (e.g. a loop-mounted image in a privileged container).
    fn test_root() -> Option<std::path::PathBuf> {
        std::env::var("LAYERFS_BTRFS_TEST_ROOT")
            .ok()
            .map(Into::into)
    }

    #[test]
    fn prepare_clone_freeze_delete_roundtrip() {
        let Some(root) = test_root() else {
            eprintln!("skipping: LAYERFS_BTRFS_TEST_ROOT not set");
            return;
        };
        let backend = BtrfsBackend::new(&root);

        let src = std::env::temp_dir().join(format!("btrfs-src-{}", std::process::id()));
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("file.txt"), "hello").unwrap();

        let base = root.join("base");
        let _ = fs::remove_dir_all(&base);
        backend.prepare_layer(&base, Some(&src)).unwrap();
        assert_eq!(fs::read_to_string(base.join("file.txt")).unwrap(), "hello");
        backend.verify_layer(&base).unwrap();

        let clone = root.join("clone");
        let _ = fs::remove_dir_all(&clone);
        backend.clone_layer(&base, &clone).unwrap();
        assert_eq!(fs::read_to_string(clone.join("file.txt")).unwrap(), "hello");

        backend.freeze_layer(&clone).unwrap();
        assert!(fs::write(clone.join("newfile"), "x").is_err());

        backend.delete_layer(&clone).unwrap();
        assert!(!clone.exists());

        backend.activate_state(&[&base]).unwrap();
        assert_eq!(
            fs::read_to_string(root.join(".active")).unwrap(),
            base.display().to_string()
        );

        backend.delete_layer(&base).unwrap();
        fs::remove_dir_all(&src).unwrap();
    }
}
