use std::ffi::CString;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};

use crate::device_scan;

const STORE_MOUNT: &str = "/run/layerfs-store";
const DISK_BY_DIR: &str = "/dev/disk";
/// Nothing else waits for the device to appear under `rdinit=`, so retry
/// resolution ourselves instead of racing a single attempt.
const DEVICE_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const DEVICE_WAIT_POLL: Duration = Duration::from_millis(200);

pub fn locate(
    explicit: Option<&str>,
    subvol: Option<&str>,
    luks: Option<&str>,
    luks_key: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(luks_spec) = luks {
        let device = resolve_device_spec(luks_spec)?;
        let mapper = crate::luks::unlock(&device, luks_key)?;
        return require_store(mapper, subvol);
    }
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

/// Resolves `UUID=`/`LABEL=`/`PARTUUID=` device specs to a real path,
/// retrying until `DEVICE_WAIT_TIMEOUT`.
fn resolve_device_spec(spec: &str) -> Result<PathBuf, String> {
    resolve_device_spec_with_timeout(spec, DEVICE_WAIT_TIMEOUT)
}

fn resolve_device_spec_with_timeout(spec: &str, timeout: Duration) -> Result<PathBuf, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(path) = resolve_device_spec_under(Path::new(DISK_BY_DIR), spec) {
            return Ok(path);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "timed out after {timeout:?} waiting for device: {spec}"
            ));
        }
        std::thread::sleep(DEVICE_WAIT_POLL);
    }
}

/// Tries the udev symlinks first, then falls back to `device_scan` for
/// `UUID=`/`LABEL=` (the only path that works with no udev running).
fn resolve_device_spec_under(disk_by_dir: &Path, spec: &str) -> Option<PathBuf> {
    let (by_dir, id) = if let Some(uuid) = spec.strip_prefix("UUID=") {
        ("by-uuid", uuid)
    } else if let Some(label) = spec.strip_prefix("LABEL=") {
        ("by-label", label)
    } else if let Some(uuid) = spec.strip_prefix("PARTUUID=") {
        ("by-partuuid", uuid)
    } else {
        return Some(PathBuf::from(spec));
    };

    let link = disk_by_dir.join(by_dir).join(id);
    if let Ok(resolved) = std::fs::canonicalize(&link) {
        return Some(resolved);
    }

    match by_dir {
        "by-uuid" => device_scan::find_by_uuid(id),
        "by-label" => device_scan::find_by_label(id),
        _ => None,
    }
}

fn require_store(path: PathBuf, subvol: Option<&str>) -> Result<PathBuf, String> {
    if subvol.is_none() && path.join("base").is_dir() {
        return Ok(path);
    }

    mount_btrfs_store(&path, subvol)
}

fn mount_btrfs_store(source: &Path, subvol: Option<&str>) -> Result<PathBuf, String> {
    let target = Path::new(STORE_MOUNT);
    mount_btrfs(source, target, subvol)?;

    if target.join("base").is_dir() {
        return Ok(target.to_path_buf());
    }

    unmount(target, UnmountFlags::empty())
        .map_err(io::Error::from)
        .map_err(|e| format!("unmount {}: {e}", target.display()))?;
    Err(format!("{} is not a LayerFS Btrfs store", source.display()))
}

/// Mounts a Btrfs device with no assumption `base` already exists there.
pub(crate) fn mount_btrfs(
    source: &Path,
    target: &Path,
    subvol: Option<&str>,
) -> Result<(), String> {
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
    .map_err(|e| format!("mount {} at {}: {e}", source.display(), target.display()))
}

pub(crate) fn resolve_device(spec: &str) -> Result<PathBuf, String> {
    resolve_device_spec(spec)
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

        assert_eq!(
            locate(Some(root.to_str().unwrap()), None, None, None).unwrap(),
            root
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires LAYERFS_BTRFS_STORE_DEVICE and CAP_SYS_ADMIN"]
    fn mounts_a_btrfs_device_store() {
        let source = std::env::var("LAYERFS_BTRFS_STORE_DEVICE").unwrap();
        let store = locate(Some(&source), None, None, None).unwrap();
        assert!(store.join("base").is_dir());
        unmount(Path::new(STORE_MOUNT), UnmountFlags::empty()).unwrap();
    }

    #[test]
    #[ignore = "requires LAYERFS_BTRFS_STORE_DEVICE, a 'layerfs' subvolume on it, and CAP_SYS_ADMIN"]
    fn mounts_a_specific_btrfs_subvolume() {
        let source = std::env::var("LAYERFS_BTRFS_STORE_DEVICE").unwrap();
        let store = locate(Some(&source), Some("layerfs"), None, None).unwrap();
        assert!(store.join("base").is_dir());
        unmount(Path::new(STORE_MOUNT), UnmountFlags::empty()).unwrap();
    }

    #[test]
    #[ignore = "requires LAYERFS_LUKS_DEVICE, LAYERFS_LUKS_KEY, a Btrfs store inside it, and CAP_SYS_ADMIN"]
    fn unlocks_and_mounts_a_luks_backed_store() {
        let device = std::env::var("LAYERFS_LUKS_DEVICE").unwrap();
        let key = std::env::var("LAYERFS_LUKS_KEY").unwrap();

        let store = locate(None, None, Some(&device), Some(&key)).unwrap();

        assert!(store.join("base").is_dir());
        unmount(Path::new(STORE_MOUNT), UnmountFlags::empty()).unwrap();
        std::process::Command::new("cryptsetup")
            .args(["luksClose", "layerfs-crypt"])
            .status()
            .unwrap();
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
            resolve_device_spec_under(&disk_by, "UUID=1111-2222"),
            Some(disk_by.join("fake-device"))
        );
        std::fs::remove_dir_all(disk_by).unwrap();
    }

    #[test]
    fn resolves_label_spec_via_the_udev_symlink() {
        let disk_by = fake_disk_by_dir("label");
        assert_eq!(
            resolve_device_spec_under(&disk_by, "LABEL=mylabel"),
            Some(disk_by.join("fake-device"))
        );
        std::fs::remove_dir_all(disk_by).unwrap();
    }

    #[test]
    fn resolves_partuuid_spec_via_the_udev_symlink() {
        let disk_by = fake_disk_by_dir("partuuid");
        assert_eq!(
            resolve_device_spec_under(&disk_by, "PARTUUID=aaaa-bbbb"),
            Some(disk_by.join("fake-device"))
        );
        std::fs::remove_dir_all(disk_by).unwrap();
    }

    #[test]
    fn non_spec_paths_pass_through_unchanged() {
        let disk_by = fake_disk_by_dir("passthrough");
        assert_eq!(
            resolve_device_spec_under(&disk_by, "/dev/vda"),
            Some(PathBuf::from("/dev/vda"))
        );
        std::fs::remove_dir_all(disk_by).unwrap();
    }

    #[test]
    fn unresolvable_uuid_falls_through_to_none() {
        let disk_by = fake_disk_by_dir("missing");
        assert_eq!(
            resolve_device_spec_under(&disk_by, "UUID=does-not-exist-nope"),
            None
        );
        std::fs::remove_dir_all(disk_by).unwrap();
    }

    #[test]
    fn resolve_device_spec_gives_up_after_its_timeout() {
        let err = resolve_device_spec_with_timeout(
            "UUID=definitely-does-not-exist",
            Duration::from_millis(50),
        )
        .unwrap_err();
        assert!(err.contains("timed out"));
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
