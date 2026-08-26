//! Crash-safe activation of rotating layers (UPDATE, UPDATE_HEAD) via
//! symlink indirection.
//!
//! `<store>/update` and `<store>/update-head` are symlinks into
//! `<store>/generations/`, never the content directories themselves.
//! Swapping a symlink's target is a single atomic `rename(2)`, so
//! activating a newly staged generation can never leave the named pointer
//! missing or pointing at a half-written directory — matching the
//! never-mutate-active-state-in-place rule (section 28).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const GENERATIONS_DIR: &str = "generations";

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Allocates a fresh, not-yet-existing directory under
/// `<store>/generations/` for staging a new `prefix` generation (e.g.
/// `"update"` or `"head"`). Does not create the directory itself — callers
/// populate it via `StorageBackend::prepare_layer`/`clone_layer`.
pub fn new_generation_path(store_root: &Path, prefix: &str) -> io::Result<PathBuf> {
    let dir = store_root.join(GENERATIONS_DIR);
    fs::create_dir_all(&dir)?;

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let seq = SEQUENCE.fetch_add(1, Ordering::Relaxed);

    Ok(dir.join(format!("{prefix}-{nanos}-{seq}")))
}

/// Atomically points the `<store>/<name>` symlink at `target`.
///
/// Builds a new symlink under a temporary name and `rename(2)`s it over
/// the old one; the old symlink (if any) is left dangling from the
/// filesystem's perspective only for the instant between the two syscalls,
/// and `<name>` itself is never briefly absent.
pub fn activate(store_root: &Path, name: &str, target: &Path) -> io::Result<()> {
    let link_path = store_root.join(name);
    let tmp_path = store_root.join(format!(".{name}.next"));

    let _ = fs::remove_file(&tmp_path);
    std::os::unix::fs::symlink(target, &tmp_path)?;
    fs::rename(&tmp_path, &link_path)?;

    Ok(())
}

/// Reads what `<store>/<name>` currently points at, if it exists at all
/// (as a symlink or, for back-compat with a hand-built store, a plain
/// directory).
pub fn current(store_root: &Path, name: &str) -> Option<PathBuf> {
    let path = store_root.join(name);
    path.is_dir().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("layerfs-generations-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn activate_points_the_symlink_at_the_target() {
        let store = scratch("activate");
        let target = store.join("generations/update-1");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("marker"), "one").unwrap();

        activate(&store, "update", &target).unwrap();

        let resolved = current(&store, "update").unwrap();
        assert_eq!(fs::read_to_string(resolved.join("marker")).unwrap(), "one");

        fs::remove_dir_all(&store).unwrap();
    }

    #[test]
    fn activate_can_repoint_to_a_new_generation() {
        let store = scratch("repoint");
        let gen1 = store.join("generations/update-1");
        let gen2 = store.join("generations/update-2");
        fs::create_dir_all(&gen1).unwrap();
        fs::create_dir_all(&gen2).unwrap();
        fs::write(gen1.join("marker"), "old").unwrap();
        fs::write(gen2.join("marker"), "new").unwrap();

        activate(&store, "update", &gen1).unwrap();
        activate(&store, "update", &gen2).unwrap();

        let resolved = current(&store, "update").unwrap();
        assert_eq!(fs::read_to_string(resolved.join("marker")).unwrap(), "new");

        fs::remove_dir_all(&store).unwrap();
    }

    #[test]
    fn new_generation_path_is_unique_per_call() {
        let store = scratch("unique");
        let a = new_generation_path(&store, "head").unwrap();
        let b = new_generation_path(&store, "head").unwrap();
        assert_ne!(a, b);

        fs::remove_dir_all(&store).unwrap();
    }
}
