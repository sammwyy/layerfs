use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use layerfs_adapter::bin_env_var;
use serde::{Deserialize, Serialize};

const CONFIG_FILES: &[&str] = &["etc/pacman.conf", "etc/pacman.d/mirrorlist"];
/// Under `/run`, which every transaction bind-mounts in — see the dnf
/// adapter's equivalent constant for why this isn't an env var.
const BRIDGE_PATH: &str = "/run/layerfs-pacman-manifest-apply.json";

/// Pacman's keyring (`/etc/pacman.d/gnupg`) is a binary GPG database, not
/// plain text files like dnf's/apt's — restoring it isn't handled here.
#[derive(Serialize, Deserialize)]
struct Manifest {
    packages: Vec<String>,
    config: BTreeMap<String, String>,
}

/// Reads `LAYERFS_LIVE_ROOT` so tests can point this at a stand-in root.
pub fn export() -> Result<String, String> {
    let live_root =
        PathBuf::from(env::var("LAYERFS_LIVE_ROOT").unwrap_or_else(|_| "/".to_string()));

    let mut config = BTreeMap::new();
    for path in CONFIG_FILES {
        if let Ok(content) = fs::read_to_string(live_root.join(path)) {
            config.insert((*path).to_string(), content);
        }
    }

    let manifest = Manifest {
        packages: explicitly_installed_packages(&live_root)?,
        config,
    };
    serde_json::to_string(&manifest).map_err(|e| e.to_string())
}

fn explicitly_installed_packages(live_root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("pacman")
        .arg("--root")
        .arg(live_root)
        .args(["-Qqe"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("pacman -Qqe failed: {}", output.status));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

pub fn apply_outer() -> ExitCode {
    let store_root = PathBuf::from(
        env::var("LAYERFS_STORE").unwrap_or_else(|_| "/run/layerfs-store".to_string()),
    );
    let manifest_path = store_root.join("manifest/pacman.json");
    let content = match fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("pacman: read {}: {e}", manifest_path.display());
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = fs::write(BRIDGE_PATH, &content) {
        eprintln!("pacman: write {BRIDGE_PATH}: {e}");
        return ExitCode::FAILURE;
    }

    let layerctl = env::var("LAYERFS_LAYERCTL_BIN").unwrap_or_else(|_| "layerctl".to_string());
    let status = Command::new(&layerctl)
        .arg("--store")
        .arg(&store_root)
        .arg("transaction")
        .arg("--")
        .arg("layerfs-pacman")
        .arg("--layerfs-manifest-apply-inner")
        .status();
    let _ = fs::remove_file(BRIDGE_PATH);

    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().unwrap_or(1).clamp(1, 255) as u8),
        Err(e) => {
            eprintln!("pacman: failed to run {layerctl}: {e}");
            ExitCode::FAILURE
        }
    }
}

pub fn apply_inner() -> ExitCode {
    let content = fs::read_to_string(BRIDGE_PATH).unwrap_or_default();
    let manifest: Manifest = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("pacman: invalid manifest: {e}");
            return ExitCode::FAILURE;
        }
    };

    for (path, content) in &manifest.config {
        let dest = Path::new("/").join(path);
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&dest, content) {
            eprintln!("pacman: restore {}: {e}", dest.display());
            return ExitCode::FAILURE;
        }
    }

    let bin = env::var(bin_env_var("pacman")).unwrap_or_else(|_| "pacman.layerfs-real".to_string());
    let err = Command::new(bin)
        .args(["-S", "--noconfirm", "--needed"])
        .args(&manifest.packages)
        .exec();
    eprintln!("pacman: failed to exec pacman -S: {err}");
    ExitCode::FAILURE
}
