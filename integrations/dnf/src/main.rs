//! Stands in for `dnf` itself so `sudo dnf install foo` transparently
//! becomes a system transaction. See `layerfs-adapter` for the shared
//! passthrough/transaction runner this only supplies classification for.

mod classify;
mod manifest;

use std::process::ExitCode;

use layerfs_adapter::Adapter;

const ADAPTER: Adapter = Adapter {
    name: "dnf",
    default_binary: "dnf.layerfs-real",
};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--layerfs-manifest-apply") => manifest::apply_outer(),
        Some("--layerfs-manifest-apply-inner") => manifest::apply_inner(),
        _ => ADAPTER.run(classify::is_mutating, manifest::export),
    }
}
