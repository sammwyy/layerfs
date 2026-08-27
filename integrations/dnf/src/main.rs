//! Stands in for `dnf` itself so `sudo dnf install foo` transparently
//! becomes a system transaction. See `layerfs-adapter` for the shared
//! passthrough/transaction runner this only supplies classification for.

mod classify;

use std::process::ExitCode;

use layerfs_adapter::Adapter;

const ADAPTER: Adapter = Adapter {
    name: "dnf",
    default_binary: "dnf.layerfs-real",
};

fn main() -> ExitCode {
    ADAPTER.run(classify::is_mutating)
}
