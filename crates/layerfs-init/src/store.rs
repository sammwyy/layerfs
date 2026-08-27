use std::ffi::CString;
use std::io;
use std::path::{Path, PathBuf};

use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};

const STORE_MOUNT: &str = "/run/layerfs-store";
const DISK_BY_DIR: &str = "/dev/disk";

pub fn locate(explicit: Option<&str>, subvol: Option<&str>) -> Result<PathBuf, String> {
    if let Some(store) = explicit {
        return require_store(resolve_device_spec(store)?, subvol);
    }

    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").map_err(|e| e.to_string())?;
    for mount in mountpoints(&mountinfo) {
        if mount.join("base").is_dir() {
            return Ok(mount);
        }
    }

    Err("no mounted LayerFS store found; pass layerfs.store=<path>".to_string())
}

/// Resolves `layerfs.store=UUID=<uuid>` / `LABEL=<label>` / `PARTUUID=<uuid>`
/// device specs to a real block device path via the standard udev symlinks
/// under `/dev/disk/by-*`, the same convention `root=UUID=...` uses. A spec
/// that isn't one of these forms (an already-mounted path, or a literal
/// device path like `/dev/vda`) passes through unchanged.
fn resolve_device_spec(spec: &str) -> Result<PathBuf, String> {
    resolve_device_spec_under(Path::new(DISK_BY_DIR), spec)
}

fn resolve_device_spec_under(disk_by_dir: &Path, spec: &str) -> Result<PathBuf, String> {
    let (by_dir, id) = if let Some(uuid) = spec.strip_prefix("UUID=") {
        ("by-uuid", uuid)
    } else if let Some(label) = spec.strip_prefix("LABEL=") {
        ("by-label", label)
    } else if let Some(uuid) = spec.strip_prefix("PARTUUID=") {
        ("by-partuuid", uuid)
    } else {
        return Ok(PathBuf::from(spec));
    };

    let link = disk_by_dir.join(by_dir).join(id);
    std::fs::canonicalize(&link).map_err(|e| format!("resolve {spec} ({}): {e}", link.display()))
}

fn require_store(path: PathBuf, subvol: Option<&str>) -> Result<PathBuf, String> {
    if subvol.is_none() && path.join("base").is_dir() {
        return Ok(path);
    }

    mount_btrfs_store(&path, subvol)
}

fn mount_btrfs_store(source: &Path, subvol: Option<&str>) -> Result<PathBuf, String> {
    let target = Path::new(STORE_MOUNT);
    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;

    let data = subvol
        .map(|name| CString::new(format!("subvol={name}")))
        .transpose()
        .map_err(|e| e.to_string())?;
    mount(
        source,
        target,
        "btrfs",
        MountFlags::empty(),
        data.as_deref(),
    )
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

        assert_eq!(locate(Some(root.to_str().unwrap()), None).unwrap(), root);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires LAYERFS_BTRFS_STORE_DEVICE and CAP_SYS_ADMIN"]
    fn mounts_a_btrfs_device_store() {
        let source = std::env::var("LAYERFS_BTRFS_STORE_DEVICE").unwrap();
        let store = locate(Some(&source), None).unwrap();
        assert!(store.join("base").is_dir());
        unmount(Path::new(STORE_MOUNT), UnmountFlags::empty()).unwrap();
    }

    #[test]
    #[ignore = "requires LAYERFS_BTRFS_STORE_DEVICE, a 'layerfs' subvolume on it, and CAP_SYS_ADMIN"]
    fn mounts_a_specific_btrfs_subvolume() {
        let source = std::env::var("LAYERFS_BTRFS_STORE_DEVICE").unwrap();
        let store = locate(Some(&source), Some("layerfs")).unwrap();
        assert!(store.join("base").is_dir());
        unmount(Path::new(STORE_MOUNT), UnmountFlags::empty()).unwrap();
    }

    fn fake_disk_by_dir(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("layerfs-disk-by-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("by-uuid")).unwrap();
        std::fs::create_dir_all(root.join("by-label")).unwrap();
        std::fs::create_dir_all(root.join("by-partuuid")).unwrap();
        std::fs::write(root.join("fake-device"), b"").unwrap();
        std::os::unix::fs::symlink("../fake-device", root.join("by-uuid/1111-2222")).unwrap();
        std::os::unix::fs::symlink("../fake-device", root.join("by-label/mylabel")).unwrap();
        std::os::unix::fs::symlink("../fake-device", root.join("by-partuuid/aaaa-bbbb")).unwrap();
        root
    }

    #[test]
    fn resolves_uuid_spec_via_the_udev_symlink() {
        let disk_by = fake_disk_by_dir("uuid");
        assert_eq!(
            resolve_device_spec_under(&disk_by, "UUID=1111-2222").unwrap(),
            disk_by.join("fake-device")
        );
        std::fs::remove_dir_all(disk_by).unwrap();
    }

    #[test]
    fn resolves_label_spec_via_the_udev_symlink() {
        let disk_by = fake_disk_by_dir("label");
        assert_eq!(
            resolve_device_spec_under(&disk_by, "LABEL=mylabel").unwrap(),
            disk_by.join("fake-device")
        );
        std::fs::remove_dir_all(disk_by).unwrap();
    }

    #[test]
    fn resolves_partuuid_spec_via_the_udev_symlink() {
        let disk_by = fake_disk_by_dir("partuuid");
        assert_eq!(
            resolve_device_spec_under(&disk_by, "PARTUUID=aaaa-bbbb").unwrap(),
            disk_by.join("fake-device")
        );
        std::fs::remove_dir_all(disk_by).unwrap();
    }

    #[test]
    fn non_spec_paths_pass_through_unchanged() {
        let disk_by = fake_disk_by_dir("passthrough");
        assert_eq!(
            resolve_device_spec_under(&disk_by, "/dev/vda").unwrap(),
            PathBuf::from("/dev/vda")
        );
        std::fs::remove_dir_all(disk_by).unwrap();
    }

    #[test]
    fn unresolvable_uuid_is_a_clear_error() {
        let disk_by = fake_disk_by_dir("missing");
        let err = resolve_device_spec_under(&disk_by, "UUID=does-not-exist").unwrap_err();
        assert!(err.contains("UUID=does-not-exist"));
        std::fs::remove_dir_all(disk_by).unwrap();
    }

    #[test]
    fn resolves_a_real_uuid_from_the_actual_host_disk_layout() {
        let Ok(entries) = std::fs::read_dir("/dev/disk/by-uuid") else {
            return;
        };
        let Some(entry) = entries.filter_map(|e| e.ok()).next() else {
            return;
        };
        let uuid = entry.file_name().into_string().unwrap();
        let expected = std::fs::canonicalize(entry.path()).unwrap();

        assert_eq!(
            resolve_device_spec(&format!("UUID={uuid}")).unwrap(),
            expected
        );
    }
}
