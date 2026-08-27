use std::ffi::CStr;

use rustix::mount::{MountFlags, mount};

/// Mounts proc/sysfs/devtmpfs. As `rdinit=`, nothing else has run yet to
/// do this — not even the initramfs's own build system's `/init`.
pub fn mount_pseudo_fs() {
    let no_data: Option<&CStr> = None;
    for dir in ["/proc", "/sys", "/dev"] {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = mount("proc", "/proc", "proc", MountFlags::empty(), no_data);
    let _ = mount("sysfs", "/sys", "sysfs", MountFlags::empty(), no_data);
    let _ = mount("devtmpfs", "/dev", "devtmpfs", MountFlags::empty(), no_data);
}
