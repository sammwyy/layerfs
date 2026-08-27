use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use layerfs_adapter::bin_env_var;
use serde::{Deserialize, Serialize};

const REPO_DIR: &str = "etc/yum.repos.d";
const KEY_DIR: &str = "etc/pki/rpm-gpg";
/// Under `/run` (bind-mounted into every transaction) — an env var hits
/// ARG_MAX on a manifest with many repo files.
const BRIDGE_PATH: &str = "/run/layerfs-dnf-manifest-apply.json";

#[derive(Serialize, Deserialize)]
struct Manifest {
    packages: Vec<String>,
    repos: BTreeMap<String, String>,
    keys: BTreeMap<String, String>,
}

/// Reads `LAYERFS_LIVE_ROOT` so tests can point this at a stand-in root.
pub fn export() -> Result<String, String> {
    let live_root =
        PathBuf::from(env::var("LAYERFS_LIVE_ROOT").unwrap_or_else(|_| "/".to_string()));

    let manifest = Manifest {
        packages: userinstalled_packages(&live_root)?,
        repos: read_dir_files(&live_root.join(REPO_DIR)),
        keys: read_dir_files(&live_root.join(KEY_DIR)),
    };
    serde_json::to_string(&manifest).map_err(|e| e.to_string())
}

fn userinstalled_packages(live_root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("dnf")
        .arg("--installroot")
        .arg(live_root)
        .args(["repoquery", "--userinstalled", "--qf", "%{name}\n"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("dnf repoquery failed: {}", output.status));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

fn read_dir_files(dir: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|t| t.is_file())
            && let Ok(content) = fs::read_to_string(entry.path())
        {
            files.insert(entry.file_name().to_string_lossy().into_owned(), content);
        }
    }
    files
}

/// Reads `manifest/dnf.json`, then spawns `layerctl transaction` to apply
/// it — chrooting happens inside that transaction, not here.
pub fn apply_outer() -> ExitCode {
    let store_root = PathBuf::from(
        env::var("LAYERFS_STORE").unwrap_or_else(|_| "/run/layerfs-store".to_string()),
    );
    let manifest_path = store_root.join("manifest/dnf.json");
    let content = match fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dnf: read {}: {e}", manifest_path.display());
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = fs::write(BRIDGE_PATH, &content) {
        eprintln!("dnf: write {BRIDGE_PATH}: {e}");
        return ExitCode::FAILURE;
    }

    let layerctl = env::var("LAYERFS_LAYERCTL_BIN").unwrap_or_else(|_| "layerctl".to_string());
    let status = Command::new(&layerctl)
        .arg("--store")
        .arg(&store_root)
        .arg("transaction")
        .arg("--")
        .arg("layerfs-dnf")
        .arg("--layerfs-manifest-apply-inner")
        .status();
    let _ = fs::remove_file(BRIDGE_PATH);

    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().unwrap_or(1).clamp(1, 255) as u8),
        Err(e) => {
            eprintln!("dnf: failed to run {layerctl}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Runs chrooted inside the transaction: restores repos/keys, then execs
/// the real `dnf install` for every package in the manifest.
pub fn apply_inner() -> ExitCode {
    let content = fs::read_to_string(BRIDGE_PATH).unwrap_or_default();
    let manifest: Manifest = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("dnf: invalid manifest: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = write_dir_files(&Path::new("/").join(REPO_DIR), &manifest.repos) {
        eprintln!("dnf: restore repos: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(e) = write_dir_files(&Path::new("/").join(KEY_DIR), &manifest.keys) {
        eprintln!("dnf: restore keys: {e}");
        return ExitCode::FAILURE;
    }

    let bin = env::var(bin_env_var("dnf")).unwrap_or_else(|_| "dnf.layerfs-real".to_string());
    let err = Command::new(bin)
        .arg("install")
        .arg("-y")
        .args(&manifest.packages)
        .exec();
    eprintln!("dnf: failed to exec dnf install: {err}");
    ExitCode::FAILURE
}

fn write_dir_files(dir: &Path, files: &BTreeMap<String, String>) -> Result<(), String> {
    if files.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for (name, content) in files {
        fs::write(dir.join(name), content).map_err(|e| e.to_string())?;
    }
    Ok(())
}
