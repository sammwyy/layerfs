use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::generations;

pub const KERNEL_FILENAME: &str = "vmlinuz";
pub const INITRAMFS_FILENAME: &str = "initramfs.img";

/// Currently active boot artifact generations, mirroring root's
/// BASE/UPDATE/UPDATE_HEAD: `base` is the factory-recovery kernel, `head`
/// the newest, `update` the one before it.
#[derive(Debug, Clone, Default)]
pub struct BootArtifacts {
    pub base: Option<PathBuf>,
    pub update: Option<PathBuf>,
    pub head: Option<PathBuf>,
}

pub fn discover(boot_store: &Path) -> BootArtifacts {
    BootArtifacts {
        base: generations::current(boot_store, "base"),
        update: generations::current(boot_store, "update"),
        head: generations::current(boot_store, "head"),
    }
}

/// Registers `kernel`/`initramfs` as the new `name` generation (`"base"`,
/// `"update"`, or `"head"`) and activates it atomically.
pub fn register(
    boot_store: &Path,
    name: &str,
    kernel: &Path,
    initramfs: &Path,
) -> io::Result<PathBuf> {
    let dest = generations::new_generation_path(boot_store, name)?;
    fs::create_dir_all(&dest)?;
    fs::copy(kernel, dest.join(KERNEL_FILENAME))?;
    fs::copy(initramfs, dest.join(INITRAMFS_FILENAME))?;
    generations::activate(boot_store, name, &dest)?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("layerfs-boot-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn register_then_discover_round_trips() {
        let store = scratch("register");
        let kernel = store.join("fake-vmlinuz");
        let initramfs = store.join("fake-initramfs");
        fs::write(&kernel, "kernel-bytes").unwrap();
        fs::write(&initramfs, "initramfs-bytes").unwrap();

        register(&store, "base", &kernel, &initramfs).unwrap();

        let found = discover(&store);
        let base = found.base.unwrap();
        assert_eq!(
            fs::read_to_string(base.join(KERNEL_FILENAME)).unwrap(),
            "kernel-bytes"
        );
        assert_eq!(
            fs::read_to_string(base.join(INITRAMFS_FILENAME)).unwrap(),
            "initramfs-bytes"
        );
        assert!(found.update.is_none());
        assert!(found.head.is_none());

        fs::remove_dir_all(&store).unwrap();
    }

    #[test]
    fn registering_again_repoints_atomically() {
        let store = scratch("repoint");
        let kernel = store.join("k");
        let initramfs = store.join("i");
        fs::write(&kernel, "v1").unwrap();
        fs::write(&initramfs, "v1").unwrap();
        register(&store, "head", &kernel, &initramfs).unwrap();

        fs::write(&kernel, "v2").unwrap();
        fs::write(&initramfs, "v2").unwrap();
        register(&store, "head", &kernel, &initramfs).unwrap();

        let head = discover(&store).head.unwrap();
        assert_eq!(
            fs::read_to_string(head.join(KERNEL_FILENAME)).unwrap(),
            "v2"
        );

        fs::remove_dir_all(&store).unwrap();
    }
}
