//! Standalone binary that prints the five LayerFS checkpoint GRUB entries.
//!
//! Installable directly as an executable `/etc/grub.d/` script (e.g.
//! `41_layerfs`): `grub2-mkconfig` runs every script in that directory and
//! concatenates its stdout into `grub.cfg`, and doesn't care whether the
//! script is shell or a compiled binary as long as its output is valid
//! GRUB configuration syntax.

mod entries;

use std::process::ExitCode;

use entries::Options;

fn main() -> ExitCode {
    let mut linux = None;
    let mut initrd = None;
    let mut store = None;
    let mut extra_cmdline = String::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--linux" => linux = args.next(),
            "--initrd" => initrd = args.next(),
            "--store" => store = args.next(),
            "--extra-cmdline" => extra_cmdline = args.next().unwrap_or_default(),
            other => {
                eprintln!("layerfs-grub-entries: unknown argument '{other}'");
                return ExitCode::FAILURE;
            }
        }
    }

    let (Some(linux), Some(initrd), Some(store)) = (linux, initrd, store) else {
        eprintln!(
            "usage: layerfs-grub-entries --linux <path> --initrd <path> --store <path> [--extra-cmdline <params>]"
        );
        return ExitCode::FAILURE;
    };

    print!(
        "{}",
        entries::render(&Options {
            linux,
            initrd,
            store,
            extra_cmdline,
        })
    );
    ExitCode::SUCCESS
}
