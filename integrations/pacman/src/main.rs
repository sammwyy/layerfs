//! Stands in for `pacman` itself so `sudo pacman -S foo` transparently
//! becomes a system transaction. See `layerfs-adapter` for the shared
//! passthrough/transaction runner this only supplies classification for.

mod classify;

use std::process::ExitCode;

use layerfs_adapter::Adapter;

const ADAPTER: Adapter = Adapter {
    name: "pacman",
    default_binary: "pacman",
};

fn main() -> ExitCode {
    ADAPTER.run(classify::is_mutating)
}
