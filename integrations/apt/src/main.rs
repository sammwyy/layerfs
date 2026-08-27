//! Stands in for `apt`/`apt-get` itself so `sudo apt install foo`
//! transparently becomes a system transaction. See `layerfs-adapter` for
//! the shared passthrough/transaction runner this only classifies for.

mod classify;

use std::process::ExitCode;

use layerfs_adapter::Adapter;

const ADAPTER: Adapter = Adapter {
    name: "apt",
    default_binary: "apt-get.layerfs-real",
};

fn main() -> ExitCode {
    ADAPTER.run(classify::is_mutating)
}
