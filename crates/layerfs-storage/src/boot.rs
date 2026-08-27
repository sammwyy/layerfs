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

/// Looks for a kernel a package transaction just wrote directly into `root`
/// (a fresh, empty-before-the-transaction upper layer, so anything here was
/// written by this transaction, not inherited): the most recently modified
/// `boot/vmlinuz-<version>` with a matching `boot/initramfs-<version>.img`.
/// `None` means the transaction didn't touch `/boot` at all.
pub fn find_new_kernel(root: &Path) -> Option<(PathBuf, PathBuf)> {
    let boot_dir = root.join("boot");
    let newest = fs::read_dir(&boot_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("vmlinuz-"))
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            Some((e.path(), modified))
        })
        .max_by_key(|(_, modified)| *modified)?;
    let kernel = newest.0;

    let version = kernel.file_name()?.to_str()?.strip_prefix("vmlinuz-")?;
    let initramfs = boot_dir.join(format!("initramfs-{version}.img"));
    initramfs.exists().then_some((kernel, initramfs))
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
    fn find_new_kernel_pairs_matching_version() {
        let root = scratch("find-new-kernel");
        fs::create_dir_all(root.join("boot")).unwrap();
        fs::write(root.join("boot/vmlinuz-6.1.0"), "kernel").unwrap();
        fs::write(root.join("boot/initramfs-6.1.0.img"), "initramfs").unwrap();

        let (kernel, initramfs) = find_new_kernel(&root).unwrap();

        assert_eq!(kernel, root.join("boot/vmlinuz-6.1.0"));
        assert_eq!(initramfs, root.join("boot/initramfs-6.1.0.img"));

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn find_new_kernel_ignores_vmlinuz_without_matching_initramfs() {
        let root = scratch("find-new-kernel-orphan");
        fs::create_dir_all(root.join("boot")).unwrap();
        fs::write(root.join("boot/vmlinuz-6.1.0"), "kernel").unwrap();

        assert!(find_new_kernel(&root).is_none());

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn find_new_kernel_picks_most_recently_modified() {
        let root = scratch("find-new-kernel-newest");
        fs::create_dir_all(root.join("boot")).unwrap();
        fs::write(root.join("boot/vmlinuz-old"), "old").unwrap();
        fs::write(root.join("boot/initramfs-old.img"), "old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(root.join("boot/vmlinuz-new"), "new").unwrap();
        fs::write(root.join("boot/initramfs-new.img"), "new").unwrap();

        let (kernel, _) = find_new_kernel(&root).unwrap();

        assert_eq!(kernel, root.join("boot/vmlinuz-new"));

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn find_new_kernel_none_when_boot_untouched() {
        let root = scratch("find-new-kernel-untouched");
        fs::create_dir_all(&root).unwrap();

        assert!(find_new_kernel(&root).is_none());

        fs::remove_dir_all(&root).unwrap();
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
