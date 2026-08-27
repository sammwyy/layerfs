use std::env;
use std::path::Path;
use std::process::Command;

use layerfs_core::BootOptions;

use crate::{log, store};

const SOURCE_MOUNT: &str = "/mnt/migrate-source";
const STORE_MOUNT: &str = "/mnt/migrate-store";
const DEFAULT_LAYERCTL: &str = "/usr/bin/layerctl";

/// Converts `migrate_source` into a fresh LayerFS store on `store` by
/// reusing `layerctl install` (this initramfs is single-purpose).
pub fn run(opts: &BootOptions) -> ! {
    let source_spec = opts
        .migrate_source
        .as_deref()
        .unwrap_or_else(|| log::fatal("layerfs.migrate=1 requires layerfs.migrate_source="));
    let store_spec = opts
        .store
        .as_deref()
        .unwrap_or_else(|| log::fatal("layerfs.migrate=1 requires layerfs.store="));

    let source_device = store::resolve_device(source_spec)
        .unwrap_or_else(|e| log::fatal(&format!("resolve migrate_source: {e}")));
    let store_device = store::resolve_device(store_spec)
        .unwrap_or_else(|e| log::fatal(&format!("resolve store: {e}")));

    mount_source_ro(&source_device);
    store::mount_btrfs(
        &store_device,
        Path::new(STORE_MOUNT),
        opts.subvol.as_deref(),
    )
    .unwrap_or_else(|e| log::fatal(&format!("mount store: {e}")));

    let layerctl =
        env::var("LAYERFS_LAYERCTL_BIN").unwrap_or_else(|_| DEFAULT_LAYERCTL.to_string());
    log::info("running layerctl install");
    let status = Command::new(&layerctl)
        .arg("--store")
        .arg(STORE_MOUNT)
        .arg("install")
        .arg("--source")
        .arg(SOURCE_MOUNT)
        .status();

    match status {
        Ok(s) if s.success() => {
            log::info("migration complete; store ready");
            log::info(
                "this boot entry still points here — repoint the boot loader before rebooting",
            );
            unmount_all();
            let _ = rustix::system::reboot(rustix::system::RebootCommand::PowerOff);
            log::fatal("reboot(2) returned");
        }
        Ok(s) => log::fatal(&format!("layerctl install exited with {s}")),
        Err(e) => log::fatal(&format!("run layerctl install: {e}")),
    }
}

/// `btrfs` commits metadata lazily; an unclean power-off before unmounting
/// can lose writes that `layerctl install` already reported as done.
fn unmount_all() {
    use rustix::mount::{UnmountFlags, unmount};
    let _ = unmount(STORE_MOUNT, UnmountFlags::empty());
    let _ = unmount(SOURCE_MOUNT, UnmountFlags::empty());
}

/// Shells out to the real `mount(8)` for its filesystem autodetection —
/// the source could be ext4, xfs, btrfs, anything `mount -t auto` finds.
fn mount_source_ro(device: &Path) {
    std::fs::create_dir_all(SOURCE_MOUNT).unwrap_or_else(|e| log::fatal(&e.to_string()));
    let status = Command::new("mount")
        .args(["-o", "ro"])
        .arg(device)
        .arg(SOURCE_MOUNT)
        .status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => log::fatal(&format!("mount migrate_source: exited with {s}")),
        Err(e) => log::fatal(&format!("run mount: {e}")),
    }
}
