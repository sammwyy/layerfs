use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

const BUS_DEVICES: &str = "/sys/bus";
/// PID 1 has no `PATH`, and libc's fallback search path omits sbin dirs.
const MODPROBE_PATHS: [&str; 3] = ["/sbin/modprobe", "/usr/sbin/modprobe", "/bin/modprobe"];
/// A transport driver (e.g. `virtio_pci`) exposes new child bus devices
/// only after it binds, so one static scan isn't enough.
const MAX_PASSES: u8 = 4;

/// Loads whatever kernel module each bus device's `modalias` names.
/// Nothing else does this under `rdinit=` — no udev has run.
pub fn load_bus_drivers() {
    let Some(modprobe) = find_modprobe() else {
        return;
    };
    let mut loaded = HashSet::new();
    for _ in 0..MAX_PASSES {
        let seen = bus_modaliases(Path::new(BUS_DEVICES));
        let new_aliases: Vec<_> = seen.difference(&loaded).cloned().collect();
        if new_aliases.is_empty() {
            break;
        }
        for alias in &new_aliases {
            let _ = Command::new(modprobe).arg(alias).status();
        }
        loaded.extend(new_aliases);
    }
}

fn find_modprobe() -> Option<&'static str> {
    MODPROBE_PATHS
        .into_iter()
        .find(|path| Path::new(path).exists())
}

fn bus_modaliases(bus_root: &Path) -> HashSet<String> {
    let mut aliases = HashSet::new();
    let Ok(buses) = std::fs::read_dir(bus_root) else {
        return aliases;
    };
    for bus in buses.flatten() {
        let Ok(devices) = std::fs::read_dir(bus.path().join("devices")) else {
            continue;
        };
        for device in devices.flatten() {
            if let Ok(alias) = std::fs::read_to_string(device.path().join("modalias")) {
                aliases.insert(alias.trim().to_string());
            }
        }
    }
    aliases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_modalias_from_every_bus_devices_directory() {
        let root = std::env::temp_dir().join(format!("layerfs-bus-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("pci/devices/0000:00:03.0")).unwrap();
        std::fs::create_dir_all(root.join("virtio/devices/virtio0")).unwrap();
        std::fs::write(
            root.join("pci/devices/0000:00:03.0/modalias"),
            "pci:v00001AF4d00001001sv00001AF4sd00000002bc01sc00i00\n",
        )
        .unwrap();
        std::fs::write(
            root.join("virtio/devices/virtio0/modalias"),
            "virtio:d00000002v00001AF4\n",
        )
        .unwrap();

        let aliases = bus_modaliases(&root);

        assert_eq!(
            aliases,
            HashSet::from([
                "pci:v00001AF4d00001001sv00001AF4sd00000002bc01sc00i00".to_string(),
                "virtio:d00000002v00001AF4".to_string(),
            ])
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_bus_root_yields_no_aliases() {
        let root = std::env::temp_dir().join("layerfs-bus-does-not-exist");
        assert!(bus_modaliases(&root).is_empty());
    }
}
