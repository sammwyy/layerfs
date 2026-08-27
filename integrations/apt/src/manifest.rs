use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use layerfs_adapter::bin_env_var;
use serde::{Deserialize, Serialize};

const SOURCES_LIST: &str = "etc/apt/sources.list";
const SOURCES_DIR: &str = "etc/apt/sources.list.d";
const KEY_DIR: &str = "etc/apt/trusted.gpg.d";
/// Under `/run`, which every transaction bind-mounts in — see the dnf
/// adapter's equivalent constant for why this isn't an env var.
const BRIDGE_PATH: &str = "/run/layerfs-apt-manifest-apply.json";

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

    let mut repos = read_dir_files(&live_root.join(SOURCES_DIR));
    if let Ok(content) = fs::read_to_string(live_root.join(SOURCES_LIST)) {
        repos.insert("sources.list".to_string(), content);
    }

    let manifest = Manifest {
        packages: manually_installed_packages(&live_root)?,
        repos,
        keys: read_dir_files(&live_root.join(KEY_DIR)),
    };
    serde_json::to_string(&manifest).map_err(|e| e.to_string())
}

fn manually_installed_packages(live_root: &Path) -> Result<Vec<String>, String> {
    let output = if live_root == Path::new("/") {
        Command::new("apt-mark").arg("showmanual").output()
    } else {
        Command::new("chroot")
            .arg(live_root)
            .args(["apt-mark", "showmanual"])
            .output()
    }
    .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("apt-mark showmanual failed: {}", output.status));
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

pub fn apply_outer() -> ExitCode {
    let store_root = PathBuf::from(
        env::var("LAYERFS_STORE").unwrap_or_else(|_| "/run/layerfs-store".to_string()),
    );
    let manifest_path = store_root.join("manifest/apt.json");
    let content = match fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("apt: read {}: {e}", manifest_path.display());
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = fs::write(BRIDGE_PATH, &content) {
        eprintln!("apt: write {BRIDGE_PATH}: {e}");
        return ExitCode::FAILURE;
    }

    let layerctl = env::var("LAYERFS_LAYERCTL_BIN").unwrap_or_else(|_| "layerctl".to_string());
    let status = Command::new(&layerctl)
        .arg("--store")
        .arg(&store_root)
        .arg("transaction")
        .arg("--")
        .arg("layerfs-apt")
        .arg("--layerfs-manifest-apply-inner")
        .status();
    let _ = fs::remove_file(BRIDGE_PATH);

    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().unwrap_or(1).clamp(1, 255) as u8),
        Err(e) => {
            eprintln!("apt: failed to run {layerctl}: {e}");
            ExitCode::FAILURE
        }
    }
}

pub fn apply_inner() -> ExitCode {
    let content = fs::read_to_string(BRIDGE_PATH).unwrap_or_default();
    let manifest: Manifest = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("apt: invalid manifest: {e}");
            return ExitCode::FAILURE;
        }
    };

    for (name, content) in &manifest.repos {
        let dest = if name == "sources.list" {
            Path::new("/").join(SOURCES_LIST)
        } else {
            Path::new("/").join(SOURCES_DIR).join(name)
        };
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&dest, content) {
            eprintln!("apt: restore {}: {e}", dest.display());
            return ExitCode::FAILURE;
        }
    }
    if let Err(e) = write_dir_files(&Path::new("/").join(KEY_DIR), &manifest.keys) {
        eprintln!("apt: restore keys: {e}");
        return ExitCode::FAILURE;
    }

    let bin = env::var(bin_env_var("apt")).unwrap_or_else(|_| "apt-get.layerfs-real".to_string());
    let err = Command::new(bin)
        .arg("install")
        .arg("-y")
        .args(&manifest.packages)
        .exec();
    eprintln!("apt: failed to exec apt-get install: {err}");
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
