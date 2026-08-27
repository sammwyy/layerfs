use std::ffi::{CStr, CString};
use std::os::unix::process::CommandExt;
use std::process::Command;

use rustix::mount::{MountFlags, mount, mount_bind};
use rustix::system::init_module;

fn main() {
    mount_pseudo_fs();
    if let Err(error) = mount_bind("/store", "/run/layerfs-store") {
        eprintln!("QEMU-SWITCH-ROOT: FAIL: mount store: {error}");
        return;
    }
    if let Err(error) = load_overlay_module() {
        eprintln!("QEMU-SWITCH-ROOT: FAIL: loading overlay.ko: {error}");
        return;
    }

    let error = Command::new("/layerfs-init").exec();
    eprintln!("QEMU-SWITCH-ROOT: FAIL: exec layerfs-init: {error}");
}

fn mount_pseudo_fs() {
    let no_data: Option<&CStr> = None;
    let _ = mount("proc", "/proc", "proc", MountFlags::empty(), no_data);
    let _ = mount("sysfs", "/sys", "sysfs", MountFlags::empty(), no_data);
    let _ = mount("devtmpfs", "/dev", "devtmpfs", MountFlags::empty(), no_data);
    let _ = std::fs::create_dir_all("/run/layerfs-store");
}

fn load_overlay_module() -> Result<(), String> {
    let image = std::fs::read("/overlay.ko").map_err(|e| e.to_string())?;
    let params = CString::new("").unwrap();
    init_module(&image, &params).map_err(|e| e.to_string())
}
