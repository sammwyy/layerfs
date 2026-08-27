//! Boot sequence: parse options, assemble the selected root, then switch to
//! its init. Package management and networking are deliberately out of scope.

use std::fs;
use std::path::Path;

use layerfs_core::BootOptions;
use layerfs_init::{log, mount, store, switch_root};

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

    let store = match store::locate(
        opts.store.as_deref(),
        opts.subvol.as_deref(),
        opts.luks.as_deref(),
        opts.luks_key.as_deref(),
    ) {
        Ok(store) => store,
        Err(error) => log::fatal(&format!("storage discovery failed: {error}")),
    };
    let discovered = match layerfs_storage::discover(&store) {
        Ok(d) => d,
        Err(e) => log::fatal(&format!("storage discovery failed: {e}")),
    };

    let stack = mount::resolve_stack(opts.checkpoint, opts.head, &discovered);

    let target = Path::new("/sysroot");
    if let Err(e) = mount::assemble(&stack, &discovered.work, target) {
        log::fatal(&format!("root assembly failed: {e}"));
    }

    if opts.checkpoint.includes_data()
        && let Some(data_root) = &discovered.data
        && let Err(e) = mount::mount_data(data_root, target)
    {
        log::fatal(&format!("DATA mount failed: {e}"));
    }

    log::info(&format!("switching root to {}", target.display()));
    if let Err(e) = switch_root::switch_root(target, Path::new("/sbin/init")) {
        log::fatal(&format!("switch_root failed: {e}"));
    }
}
