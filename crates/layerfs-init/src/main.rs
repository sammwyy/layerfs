//! Boot sequence: read /proc/cmdline, discover LayerFS storage, resolve
//! the checkpoint, assemble OverlayFS, mount DATA, switch_root.
//!
//! Deliberately does not update packages, squash layers, touch the
//! network, or run as a daemon — boot correctness over convenience.

mod log;
mod mount;

use std::fs;

use layerfs_core::BootOptions;

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

    let stack = mount::resolve_stack(opts.checkpoint, &opts);

    if let Err(e) = mount::assemble(&stack) {
        log::fatal(&format!("root assembly failed: {e}"));
    }
}
