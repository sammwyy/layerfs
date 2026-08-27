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
    let mut esp_prefix = None;
    let mut extra_cmdline = String::new();
    let mut rdinit = None;
    let mut integrations = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--boot-store" => boot_store = args.next(),
            "--store" => store = args.next(),
            "--entries-dir" => entries_dir = args.next(),
            "--esp-prefix" => esp_prefix = args.next(),
            "--extra-cmdline" => extra_cmdline = args.next().unwrap_or_default(),
            "--rdinit" => rdinit = args.next(),
            "--integrations" => {
                integrations = args
                    .next()
                    .unwrap_or_default()
                    .split(',')
                    .filter(|name| !name.is_empty())
                    .map(String::from)
                    .collect();
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    let (Some(boot_store), Some(store), Some(entries_dir), Some(esp_prefix)) =
        (boot_store, store, entries_dir, esp_prefix)
    else {
        return Err("usage: layerfs-systemd-boot --boot-store <path> --esp-prefix <path> --store <path> --entries-dir <path> [--integrations <a,b>] [--extra-cmdline <params>] [--rdinit <path>]".to_string());
    };
    let entries_dir = PathBuf::from(entries_dir);
    std::fs::create_dir_all(&entries_dir).map_err(|e| e.to_string())?;
    let boot_store = PathBuf::from(boot_store);
    let artifacts = boot::discover(&boot_store);
    for entry in ENTRIES {
        let Some(paths) = resolve(&artifacts, entry.tier, &boot_store, Path::new(&esp_prefix))
        else {
            continue;
        };
        let mut options = format!(
            "layerfs.checkpoint={} layerfs.store={store}",
            entry.checkpoint
        );
        if entry.head_off {
            options.push_str(" layerfs.head=off");
        }
        if !integrations.is_empty() {
            options.push_str(" layerfs.integrations=");
            options.push_str(&integrations.join(","));
        }
        if let Some(rdinit) = &rdinit {
            options.push_str(&format!(" rdinit={rdinit}"));
        }
        if !extra_cmdline.is_empty() {
            options.push(' ');
            options.push_str(&extra_cmdline);
        }
        let contents = format!(
            "title {}\nlinux {}\ninitrd {}\noptions {}\n",
            entry.title, paths.kernel, paths.initramfs, options
        );
        write_entry(&entries_dir, entry.name, contents)?;
    }
    Ok(())
}

fn write_entry(entries_dir: &Path, name: &str, contents: String) -> Result<(), String> {
    let target = entries_dir.join(format!("{name}.conf"));
    let temporary = entries_dir.join(format!(".{name}.conf.new"));
    std::fs::write(&temporary, contents).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, target).map_err(|e| e.to_string())
}

fn resolve(
    artifacts: &BootArtifacts,
    tier: Tier,
    boot_store: &Path,
    esp_prefix: &Path,
) -> Option<Paths> {
    let path = match tier {
        Tier::Head => artifacts
            .head
            .as_ref()
            .or(artifacts.update.as_ref())
            .or(artifacts.base.as_ref()),
        Tier::Update => artifacts.update.as_ref().or(artifacts.base.as_ref()),
        Tier::Base => artifacts.base.as_ref(),
    }?;
    let relative = path.strip_prefix(boot_store).ok()?;
    Some(Paths {
        kernel: esp_prefix
            .join(relative)
            .join(KERNEL_FILENAME)
            .display()
            .to_string(),
        initramfs: esp_prefix
            .join(relative)
            .join(INITRAMFS_FILENAME)
            .display()
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_complete_bl_entry() {
        let dir = std::env::temp_dir().join(format!("layerfs-bls-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        write_entry(&dir, "layerfs-normal", "title LayerFS\n".to_string()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("layerfs-normal.conf")).unwrap(),
            "title LayerFS\n"
        );
        assert!(!dir.join(".layerfs-normal.conf.new").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn serializes_integrations_once() {
        let integrations = ["dnf", "apt"];
        assert_eq!(integrations.join(","), "dnf,apt");
    }
}
