//! Manual verification of `layerfs_init::mount` against a real OverlayFS
//! mount. Requires CAP_SYS_ADMIN in the mount namespace it runs in — invoke
//! under `unshare --map-root-user --mount`, never against the real root:
//!
//! ```text
//! cargo build -p layerfs-init --example overlay_smoke
//! unshare --map-root-user --mount -- \
//!     ./target/debug/examples/overlay_smoke /tmp/some-scratch-dir
//! ```
//!
//! Not a `cargo test` target: `unshare(CLONE_NEWUSER)` requires an
//! unthreaded process, which the test harness does not provide.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use layerfs_core::Checkpoint;
use layerfs_init::mount;
use rustix::mount::{UnmountFlags, unmount};

fn main() -> ExitCode {
    let Some(root) = std::env::args().nth(1) else {
        eprintln!("usage: overlay_smoke <scratch-dir>");
        return ExitCode::FAILURE;
    };
    let root = PathBuf::from(root);

    if let Err(e) = run(&root) {
        eprintln!("FAIL: {e}");
        return ExitCode::FAILURE;
    }

    println!("PASS");
    ExitCode::SUCCESS
}

fn run(root: &Path) -> Result<(), String> {
    let base = root.join("base");
    let over = root.join("override");
    let data = root.join("data");
    let merged = root.join("merged");

    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    fs::create_dir_all(&over).map_err(|e| e.to_string())?;
    fs::create_dir_all(data.join("home")).map_err(|e| e.to_string())?;
    fs::write(base.join("a.txt"), "base-a").map_err(|e| e.to_string())?;
    fs::write(base.join("b.txt"), "base-b").map_err(|e| e.to_string())?;
    fs::write(data.join("home/user.txt"), "persisted").map_err(|e| e.to_string())?;

    let discovered = layerfs_storage::discover(root).map_err(|e| e.to_string())?;

    // First mount: exercise copy-up, deletion (whiteout), and a new file.
    let stack = mount::resolve_stack(Checkpoint::Normal, true, &discovered);
    mount::assemble(&stack, &discovered.work, &merged).map_err(|e| e.to_string())?;

    fs::write(merged.join("a.txt"), "modified").map_err(|e| e.to_string())?;
    fs::remove_file(merged.join("b.txt")).map_err(|e| e.to_string())?;
    fs::write(merged.join("c.txt"), "new").map_err(|e| e.to_string())?;

    let a = fs::read_to_string(merged.join("a.txt")).map_err(|e| e.to_string())?;
    check(
        a == "modified",
        "a.txt should read the copied-up override content",
    )?;
    check(
        !merged.join("b.txt").exists(),
        "b.txt should be gone behind the whiteout",
    )?;
    check(
        merged.join("c.txt").exists(),
        "c.txt should exist as a new override file",
    )?;

    // DATA is a plain bind mount, not an overlay layer: writes through the
    // assembled root land directly in the backing DATA store.
    mount::mount_data(&data, &merged).map_err(|e| e.to_string())?;
    let home_user = fs::read_to_string(merged.join("home/user.txt")).map_err(|e| e.to_string())?;
    check(
        home_user == "persisted",
        "DATA bind mount should expose existing content",
    )?;
    fs::write(merged.join("home/new.txt"), "written-through").map_err(|e| e.to_string())?;
    check(
        data.join("home/new.txt").exists(),
        "writes through the DATA bind mount should land in the backing store directly",
    )?;
    unmount(merged.join("home"), UnmountFlags::empty()).map_err(|e| e.to_string())?;

    unmount(&merged, UnmountFlags::empty()).map_err(|e| e.to_string())?;

    // Base itself must be untouched: this was a copy-up, not an in-place edit.
    let base_a = fs::read_to_string(base.join("a.txt")).map_err(|e| e.to_string())?;
    check(base_a == "base-a", "BASE must never be modified in place")?;

    // Second mount: confirm the override changes persisted on disk.
    let stack = mount::resolve_stack(Checkpoint::Normal, true, &discovered);
    mount::assemble(&stack, &discovered.work, &merged).map_err(|e| e.to_string())?;

    let a = fs::read_to_string(merged.join("a.txt")).map_err(|e| e.to_string())?;
    check(
        a == "modified",
        "modification should persist across remounts",
    )?;
    check(
        !merged.join("b.txt").exists(),
        "deletion should persist across remounts",
    )?;

    unmount(&merged, UnmountFlags::empty()).map_err(|e| e.to_string())?;

    Ok(())
}

fn check(cond: bool, msg: &str) -> Result<(), String> {
    if cond { Ok(()) } else { Err(msg.to_string()) }
}
