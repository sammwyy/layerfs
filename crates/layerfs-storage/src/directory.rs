use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::backend::StorageBackend;
use crate::copy_tree::copy_tree;
use crate::error::StorageError;

/// Fallback backend for filesystems without native snapshotting (e.g.
/// ext4). Cloning and squashing require filesystem-level copying and are
/// less efficient than `BtrfsBackend`; correctness must not depend on this
/// being fast.
///
/// "Frozen" here means read-only permission bits, not true immutability —
/// the owning process (or root) can always undo it. Real tamper-resistance
/// is the Btrfs backend's job (read-only subvolumes), not this one's.
pub struct DirectoryBackend {
    /// Path to `.layerfs-store`.
    pub root: PathBuf,
}

impl DirectoryBackend {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl StorageBackend for DirectoryBackend {
    fn prepare_layer(&self, dest: &Path, source: Option<&Path>) -> Result<(), StorageError> {
        match source {
            Some(source) => copy_tree(source, dest).map_err(StorageError::from),
            None => fs::create_dir_all(dest).map_err(StorageError::from),
        }
    }

    fn clone_layer(&self, source: &Path, dest: &Path) -> Result<(), StorageError> {
        copy_tree(source, dest).map_err(StorageError::from)
    }

    fn freeze_layer(&self, layer: &Path) -> Result<(), StorageError> {
        set_writable_recursive(layer, false).map_err(StorageError::from)
    }

    fn delete_layer(&self, layer: &Path) -> Result<(), StorageError> {
        // A frozen layer's directories may have lost their write bit; regain
        // it before unlinking, since removing an entry needs write on its
        // parent directory, not the entry itself.
        set_writable_recursive(layer, true)?;
        fs::remove_dir_all(layer).map_err(StorageError::from)
    }

    fn activate_state(&self, layers: &[&Path]) -> Result<(), StorageError> {
        let listing = layers
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let tmp = self.root.join(".active.tmp");
        let dest = self.root.join(".active");

        fs::write(&tmp, listing)?;
        fs::rename(&tmp, &dest)?;

        Ok(())
    }

    fn verify_layer(&self, layer: &Path) -> Result<(), StorageError> {
        if !layer.is_dir() {
            return Err(StorageError::Discovery(format!(
                "{} is not a directory",
                layer.display()
            )));
        }

        fs::read_dir(layer)?;
        Ok(())
    }
}

fn set_writable_recursive(path: &Path, writable: bool) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }

    let mut perms = metadata.permissions();
    let mode = perms.mode();
    perms.set_mode(if writable {
        mode | 0o200
    } else {
        mode & !0o222
    });
    fs::set_permissions(path, perms)?;

    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            set_writable_recursive(&entry?.path(), writable)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "layerfs-directory-backend-{name}-{}",
            std::process::id()
        ));
        let _ = set_writable_recursive(&dir, true);
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn prepare_layer_without_source_creates_empty_dir() {
        let dest = scratch("prepare-empty");
        let backend = DirectoryBackend::new(dest.parent().unwrap());

        backend.prepare_layer(&dest, None).unwrap();
        assert!(dest.is_dir());

        fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    fn clone_layer_copies_contents() {
        let src = scratch("clone-src");
        let dest = scratch("clone-dest");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("file.txt"), "hi").unwrap();
        let backend = DirectoryBackend::new(&src);

        backend.clone_layer(&src, &dest).unwrap();

        assert_eq!(fs::read_to_string(dest.join("file.txt")).unwrap(), "hi");

        fs::remove_dir_all(&src).unwrap();
        fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    fn freeze_then_delete_still_succeeds() {
        let layer = scratch("freeze-delete");
        fs::create_dir_all(layer.join("sub")).unwrap();
        fs::write(layer.join("sub/file.txt"), "hi").unwrap();
        let backend = DirectoryBackend::new(&layer);

        backend.freeze_layer(&layer).unwrap();
        assert_eq!(
            fs::metadata(&layer).unwrap().permissions().mode() & 0o222,
            0,
            "frozen layer should have no write bits"
        );

        backend.delete_layer(&layer).unwrap();
        assert!(!layer.exists());
    }

    #[test]
    fn verify_layer_rejects_missing_directory() {
        let backend = DirectoryBackend::new("/nonexistent");
        assert!(
            backend
                .verify_layer(Path::new("/nonexistent/layer"))
                .is_err()
        );
    }
}
