//! Runs as `rdinit=/init` inside a real QEMU-booted kernel to prove
//! `layerfs_init::mount` works under an actual Linux boot, not just an
//! unprivileged user namespace. Mounts proc/sysfs/devtmpfs, then follows
//! the same discover → resolve_stack → assemble → mount_data sequence as
//! the real `layerfs-init` binary, verifies the assembled root, prints
//! PASS/FAIL to the console, and powers the VM off.
//!
//! Built for `x86_64-unknown-linux-musl` and packed into a throwaway
//! initramfs by `scripts/qemu-smoke.sh`; never run outside a VM.

use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;

use rustix::mount::{MountFlags, mount};
use rustix::system::{RebootCommand, init_module, reboot};

use layerfs_core::{BootOptions, Checkpoint};
use layerfs_init::mount as layerfs_mount;

fn main() {
    mount_pseudo_fs();

    // A real dracut integration resolves and includes filesystem modules
    // itself; this throwaway initramfs has none, so load overlay.ko by
    // hand before anything tries to mount an overlay.
    let result = load_overlay_module()
        .map_err(|e| format!("loading overlay.ko: {e}"))
        .and_then(|()| run());

    match &result {
        Ok(()) => println!("QEMU-SMOKE: PASS"),
        Err(e) => println!("QEMU-SMOKE: FAIL: {e}"),
    }

    // Flush before the kernel tears everything down under us.
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let _ = reboot(RebootCommand::PowerOff);
}

fn mount_pseudo_fs() {
    let no_data: Option<&CStr> = None;
    let _ = mount("proc", "/proc", "proc", MountFlags::empty(), no_data);
    let _ = mount("sysfs", "/sys", "sysfs", MountFlags::empty(), no_data);
    let _ = mount("devtmpfs", "/dev", "devtmpfs", MountFlags::empty(), no_data);
}

fn load_overlay_module() -> Result<(), String> {
    let image = fs::read("/overlay.ko").map_err(|e| e.to_string())?;
    let params = CString::new("").unwrap();
    init_module(&image, &params).map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let cmdline = fs::read_to_string("/proc/cmdline").map_err(|e| e.to_string())?;
    let opts = BootOptions::parse(&cmdline).map_err(|e| e.to_string())?;

    if opts.checkpoint != Checkpoint::Normal {
        return Err(format!(
            "expected checkpoint=normal, got {}",
            opts.checkpoint
        ));
    }

    let store = opts
        .store
        .ok_or("no layerfs.store= on the kernel command line")?;
    let discovered = layerfs_storage::discover(Path::new(&store)).map_err(|e| e.to_string())?;

    let stack = layerfs_mount::resolve_stack(opts.checkpoint, opts.head, &discovered);
    let target = Path::new("/run/layerfs/root");
    layerfs_mount::assemble(&stack, &discovered.work, target).map_err(|e| e.to_string())?;

    if let Some(data_root) = &discovered.data {
        layerfs_mount::mount_data(data_root, target).map_err(|e| e.to_string())?;
    }

    verify(target)
}

fn verify(target: &Path) -> Result<(), String> {
    let read = |rel: &str| fs::read_to_string(target.join(rel)).map_err(|e| format!("{rel}: {e}"));

    let a = read("a.txt")?;
    check(
        a == "modified",
        "a.txt should read the OVERRIDE copy-up content",
    )?;
    check(
        !target.join("b.txt").exists(),
        "b.txt should be hidden by the OVERRIDE whiteout",
    )?;

    let user = read("home/user.txt")?;
    check(
        user == "persisted",
        "DATA bind mount should expose home/user.txt",
    )?;

    Ok(())
}

fn check(cond: bool, msg: &str) -> Result<(), String> {
    if cond { Ok(()) } else { Err(msg.to_string()) }
}
