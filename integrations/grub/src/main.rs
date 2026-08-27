//! Standalone binary that prints the LayerFS checkpoint GRUB entries.
//!
//! Installable directly as an executable `/etc/grub.d/` script (e.g.
//! `41_layerfs`): `grub2-mkconfig` runs every script in that directory and
//! concatenates its stdout into `grub.cfg`, and doesn't care whether the
//! script is shell or a compiled binary as long as its output is valid
//! GRUB configuration syntax.

mod entries;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use layerfs_storage::boot::{self, BootArtifacts, INITRAMFS_FILENAME, KERNEL_FILENAME};

use entries::{BootTierPaths, Options};

fn main() -> ExitCode {
    let mut boot_store = None;
    let mut store = None;
    let mut integrations = Vec::new();
    let mut extra_cmdline = String::new();
    let mut rdinit = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--boot-store" => boot_store = args.next(),
            "--store" => store = args.next(),
            "--integrations" => {
                integrations = args
                    .next()
                    .unwrap_or_default()
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect();
            }
            "--extra-cmdline" => extra_cmdline = args.next().unwrap_or_default(),
            "--rdinit" => rdinit = args.next(),
            other => {
                eprintln!("layerfs-grub-entries: unknown argument '{other}'");
                return ExitCode::FAILURE;
            }
        }
    }

    let (Some(boot_store), Some(store)) = (boot_store, store) else {
        eprintln!(
            "usage: layerfs-grub-entries --boot-store <path> --store <path> [--integrations <a,b>] [--extra-cmdline <params>]"
        );
        return ExitCode::FAILURE;
    };

    let artifacts = boot::discover(Path::new(&boot_store));

    print!(
        "{}",
        entries::render(&Options {
            base: tier_paths(&artifacts, |a| &a.base),
            update: tier_paths(&artifacts, |a| &a.update),
            head: tier_paths(&artifacts, |a| &a.head),
            store,
            integrations,
            extra_cmdline,
            rdinit,
        })
    );
    ExitCode::SUCCESS
}

fn tier_paths(
    artifacts: &BootArtifacts,
    pick: impl Fn(&BootArtifacts) -> &Option<PathBuf>,
) -> Option<BootTierPaths> {
    let dir = pick(artifacts).as_ref()?;
    Some(BootTierPaths {
        kernel: dir.join(KERNEL_FILENAME).display().to_string(),
        initramfs: dir.join(INITRAMFS_FILENAME).display().to_string(),
    })
}
