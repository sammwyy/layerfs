use std::fs;
use std::path::Path;

use layerfs_storage::{DiscoveredStore, StorageBackend};

pub struct Check {
    description: String,
    ok: bool,
}

pub struct Report {
    checks: Vec<Check>,
}

impl Report {
    fn push(&mut self, description: impl Into<String>, ok: bool) {
        self.checks.push(Check {
            description: description.into(),
            ok,
        });
    }

    fn passed(&self) -> bool {
        self.checks.iter().all(|check| check.ok)
    }

    fn print(&self) {
        for check in &self.checks {
            println!(
                "[{}] {}",
                if check.ok { "ok" } else { "FAIL" },
                check.description
            );
        }
    }
}

pub fn run(store_root: &Path) -> Result<(), String> {
    let report = inspect(store_root);
    report.print();
    if report.passed() {
        Ok(())
    } else {
        Err("doctor found problems".to_string())
    }
}

fn inspect(store_root: &Path) -> Report {
    let mut report = Report { checks: Vec::new() };
    report.push(
        format!("store directory exists: {}", store_root.display()),
        store_root.is_dir(),
    );

    let discovered = match layerfs_storage::discover(store_root) {
        Ok(discovered) => discovered,
        Err(error) => {
            report.push(format!("layer discovery: {error}"), false);
            return report;
        }
    };
    report.push("layer discovery", true);

    let backend = layerfs_storage::detect_backend(store_root);
    inspect_layers(&mut report, &*backend, &discovered);
    inspect_generation_links(&mut report, store_root, &discovered);
    inspect_boot_artifacts(&mut report, store_root);

    for check in layerfs_storage::validate::verify_root(&discovered.base).checks {
        report.push(format!("base root: {}", check.description), check.ok);
    }

    report
}

fn inspect_layers(report: &mut Report, backend: &dyn StorageBackend, discovered: &DiscoveredStore) {
    inspect_layer(report, backend, "base", Some(&discovered.base));
    inspect_layer(report, backend, "update", discovered.update.as_deref());
    inspect_layer(
        report,
        backend,
        "update-head",
        discovered.update_head.as_deref(),
    );
    inspect_layer(
        report,
        backend,
        "override",
        discovered.r#override.as_deref(),
    );
    inspect_layer(report, backend, "data", discovered.data.as_deref());
}

fn inspect_layer(
    report: &mut Report,
    backend: &dyn StorageBackend,
    name: &str,
    path: Option<&Path>,
) {
    let Some(path) = path else {
        report.push(format!("{name} layer is absent"), true);
        return;
    };

    report.push(
        format!("{name} layer: {}", path.display()),
        backend.verify_layer(path).is_ok(),
    );
}

fn inspect_generation_links(report: &mut Report, store_root: &Path, discovered: &DiscoveredStore) {
    for (name, discovered_path) in [
        ("update", discovered.update.as_ref()),
        ("update-head", discovered.update_head.as_ref()),
    ] {
        let path = store_root.join(name);
        if path_exists(&path) {
            report.push(
                format!("{name} generation pointer resolves to a directory"),
                discovered_path.is_some(),
            );
        }
    }

    report.push(
        "update-head has an active update layer",
        discovered.update_head.is_none() || discovered.update.is_some(),
    );
}

fn inspect_boot_artifacts(report: &mut Report, store_root: &Path) {
    let boot_store = store_root.join("boot");
    let artifacts = layerfs_storage::boot::discover(&boot_store);
    for (name, generation) in [
        ("base", artifacts.base),
        ("update", artifacts.update),
        ("head", artifacts.head),
    ] {
        let pointer = boot_store.join(name);
        if path_exists(&pointer) && generation.is_none() {
            report.push(
                format!("boot {name} generation pointer resolves to a directory"),
                false,
            );
        }

        if let Some(generation) = generation {
            report.push(
                format!("boot {name} kernel exists"),
                generation
                    .join(layerfs_storage::boot::KERNEL_FILENAME)
                    .is_file(),
            );
            report.push(
                format!("boot {name} initramfs exists"),
                generation
                    .join(layerfs_storage::boot::INITRAMFS_FILENAME)
                    .is_file(),
            );
        }
    }
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("layerfs-doctor-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("base/usr/bin")).unwrap();
        fs::create_dir_all(root.join("base/etc")).unwrap();
        root
    }

    #[test]
    fn accepts_a_minimal_healthy_store() {
        let root = scratch("healthy");

        assert!(inspect(&root).passed());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_a_dangling_update_pointer() {
        let root = scratch("dangling");
        std::os::unix::fs::symlink(root.join("generations/missing"), root.join("update")).unwrap();

        assert!(!inspect(&root).passed());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_a_head_without_an_update() {
        let root = scratch("head-without-update");
        let head = root.join("generations/head");
        fs::create_dir_all(&head).unwrap();
        std::os::unix::fs::symlink(&head, root.join("update-head")).unwrap();

        assert!(!inspect(&root).passed());

        fs::remove_dir_all(root).unwrap();
    }
}
