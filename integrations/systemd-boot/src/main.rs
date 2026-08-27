use std::path::{Path, PathBuf};

use layerfs_storage::boot::{self, BootArtifacts, INITRAMFS_FILENAME, KERNEL_FILENAME};

struct Paths {
    kernel: String,
    initramfs: String,
}

#[derive(Clone, Copy)]
enum Tier {
    Head,
    Update,
    Base,
}

struct Entry {
    name: &'static str,
    title: &'static str,
    checkpoint: &'static str,
    head_off: bool,
    tier: Tier,
}

const ENTRIES: [Entry; 5] = [
    Entry {
        name: "layerfs-normal",
        title: "LayerFS Linux",
        checkpoint: "normal",
        head_off: false,
        tier: Tier::Head,
    },
    Entry {
        name: "layerfs-safe",
        title: "LayerFS Linux — Safe Mode",
        checkpoint: "safe",
        head_off: false,
        tier: Tier::Head,
    },
    Entry {
        name: "layerfs-system",
        title: "LayerFS Linux — System Only",
        checkpoint: "system",
        head_off: false,
        tier: Tier::Head,
    },
    Entry {
        name: "layerfs-previous",
        title: "LayerFS Linux — Previous Update",
        checkpoint: "safe",
        head_off: true,
        tier: Tier::Update,
    },
    Entry {
        name: "layerfs-base",
        title: "LayerFS Linux — Base Recovery",
        checkpoint: "base",
        head_off: false,
        tier: Tier::Base,
    },
];

fn main() -> Result<(), String> {
    let mut boot_store = None;
    let mut store = None;
    let mut entries_dir = None;
    let mut rdinit = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--boot-store" => boot_store = args.next(),
            "--store" => store = args.next(),
            "--entries-dir" => entries_dir = args.next(),
            "--rdinit" => rdinit = args.next(),
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    let (Some(boot_store), Some(store), Some(entries_dir)) = (boot_store, store, entries_dir)
    else {
        return Err("usage: layerfs-systemd-boot --boot-store <path> --store <path> --entries-dir <path> [--rdinit <path>]".to_string());
    };
    let entries_dir = PathBuf::from(entries_dir);
    std::fs::create_dir_all(&entries_dir).map_err(|e| e.to_string())?;
    let artifacts = boot::discover(Path::new(&boot_store));
    for entry in ENTRIES {
        let Some(paths) = resolve(&artifacts, entry.tier) else {
            continue;
        };
        let mut options = format!(
            "layerfs.checkpoint={} layerfs.store={store}",
            entry.checkpoint
        );
        if entry.head_off {
            options.push_str(" layerfs.head=off");
        }
        if let Some(rdinit) = &rdinit {
            options.push_str(&format!(" rdinit={rdinit}"));
        }
        let contents = format!(
            "title {}\nlinux {}\ninitrd {}\noptions {}\n",
            entry.title, paths.kernel, paths.initramfs, options
        );
        std::fs::write(entries_dir.join(format!("{}.conf", entry.name)), contents)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn resolve(artifacts: &BootArtifacts, tier: Tier) -> Option<Paths> {
    let path = match tier {
        Tier::Head => artifacts
            .head
            .as_ref()
            .or(artifacts.update.as_ref())
            .or(artifacts.base.as_ref()),
        Tier::Update => artifacts.update.as_ref().or(artifacts.base.as_ref()),
        Tier::Base => artifacts.base.as_ref(),
    }?;
    Some(Paths {
        kernel: path.join(KERNEL_FILENAME).display().to_string(),
        initramfs: path.join(INITRAMFS_FILENAME).display().to_string(),
    })
}
