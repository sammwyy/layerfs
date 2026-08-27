use std::io;
use std::path::{Path, PathBuf};

use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};

const STORE_MOUNT: &str = "/run/layerfs-store";

pub fn locate(explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(store) = explicit {
        return require_store(PathBuf::from(store));
    }

    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").map_err(|e| e.to_string())?;
    for mount in mountpoints(&mountinfo) {
        if mount.join("base").is_dir() {
            return Ok(mount);
        }
    }

    Err("no mounted LayerFS store found; pass layerfs.store=<path>".to_string())
}

fn require_store(path: PathBuf) -> Result<PathBuf, String> {
    if path.join("base").is_dir() {
        return Ok(path);
    }

    mount_btrfs_store(&path)
}

fn mount_btrfs_store(source: &Path) -> Result<PathBuf, String> {
    let target = Path::new(STORE_MOUNT);
    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
    let no_data = None;
    mount(source, target, "btrfs", MountFlags::empty(), no_data)
        .map_err(io::Error::from)
        .map_err(|e| format!("mount {} at {}: {e}", source.display(), target.display()))?;

    if target.join("base").is_dir() {
        return Ok(target.to_path_buf());
    }

    unmount(target, UnmountFlags::empty())
        .map_err(io::Error::from)
        .map_err(|e| format!("unmount {}: {e}", target.display()))?;
    Err(format!("{} is not a LayerFS Btrfs store", source.display()))
}

fn mountpoints(mountinfo: &str) -> impl Iterator<Item = PathBuf> + '_ {
    mountinfo.lines().filter_map(|line| {
        line.split(' ')
            .nth(4)
            .map(|mountpoint| Path::new(mountpoint).to_path_buf())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mountpoints() {
        let mounts =
            mountpoints("1 2 0:1 / / rw - rootfs rootfs rw\n3 1 0:2 / /store rw - tmpfs tmpfs rw")
                .collect::<Vec<_>>();
        assert_eq!(mounts, [PathBuf::from("/"), PathBuf::from("/store")]);
    }

    #[test]
    fn mounted_path_is_accepted_as_a_store() {
        let root = std::env::temp_dir().join(format!("layerfs-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("base")).unwrap();

        assert_eq!(locate(Some(root.to_str().unwrap())).unwrap(), root);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires LAYERFS_BTRFS_STORE_DEVICE and CAP_SYS_ADMIN"]
    fn mounts_a_btrfs_device_store() {
        let source = std::env::var("LAYERFS_BTRFS_STORE_DEVICE").unwrap();
        let store = locate(Some(&source)).unwrap();
        assert!(store.join("base").is_dir());
        unmount(Path::new(STORE_MOUNT), UnmountFlags::empty()).unwrap();
    }
}
