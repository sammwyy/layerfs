//! Boot sequence: read /proc/cmdline, discover LayerFS storage, resolve
//! the checkpoint, assemble OverlayFS, mount DATA, switch_root.
//!
//! Deliberately does not update packages, squash layers, touch the
//! network, or run as a daemon — boot correctness over convenience.

use std::fs;
use std::path::Path;

use layerfs_core::BootOptions;
use layerfs_init::{log, mount};

fn main() {
    let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();

    let opts = match BootOptions::parse(&cmdline) {
        Ok(opts) => opts,
        Err(e) => {
            log::fatal(&format!("invalid boot options ({e}), refusing to guess"));
        }
    };

    log::info(&format!("checkpoint={}", opts.checkpoint));
    log::debug(
        opts.debug,
        &format!("head={} store={:?}", opts.head, opts.store),
    );

    let Some(store) = &opts.store else {
        log::fatal("no layerfs.store= given, nothing to assemble");
    };

    let discovered = match layerfs_storage::discover(Path::new(store)) {
        Ok(d) => d,
        Err(e) => log::fatal(&format!("storage discovery failed: {e}")),
    };

    let stack = mount::resolve_stack(opts.checkpoint, opts.head, &discovered);

    // TODO: target should be /sysroot once this runs inside a real
    // initramfs; switch_root is not implemented yet.
    let target = Path::new("/run/layerfs/root");
    if let Err(e) = mount::assemble(&stack, &discovered.work, target) {
        log::fatal(&format!("root assembly failed: {e}"));
    }

    if opts.checkpoint.includes_data()
        && let Some(data_root) = &discovered.data
        && let Err(e) = mount::mount_data(data_root, target)
    {
        log::fatal(&format!("DATA mount failed: {e}"));
    }

    log::info(&format!("mounted at {}", target.display()));
}
