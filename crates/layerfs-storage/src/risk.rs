use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Paths whose changes can leave already-running processes on a stale
/// version indefinitely (shared libraries, the kernel, systemd itself) —
/// touching any of these means hot-applying isn't safe.
const RISKY_PREFIXES: &[&str] = &[
    "usr/lib",
    "usr/lib64",
    "lib",
    "lib64",
    "boot",
    "usr/lib/systemd",
];

pub fn is_risky_path(rel: &Path) -> bool {
    RISKY_PREFIXES.iter().any(|p| rel.starts_with(p))
}

/// Whether any path in `layer_dir` matches a risky prefix. Errs on the
/// side of "risky" — an update whose safety can't be determined should
/// require a reboot, not skip the check.
pub fn layer_is_risky(layer_dir: &Path) -> io::Result<bool> {
    let mut paths = Vec::new();
    walk(layer_dir, Path::new(""), &mut paths)?;
    Ok(paths.iter().any(|p| is_risky_path(p)))
}

fn walk(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root.join(rel))? {
        let entry = entry?;
        let rel_path = rel.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            walk(root, &rel_path, out)?;
        }
        out.push(rel_path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("layerfs-risk-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn safe_when_only_ordinary_binaries_change() {
        let dir = scratch("safe");
        fs::create_dir_all(dir.join("usr/bin")).unwrap();
        fs::write(dir.join("usr/bin/example"), "v2").unwrap();

        assert!(!layer_is_risky(&dir).unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn risky_when_a_shared_library_changes() {
        let dir = scratch("risky-lib");
        fs::create_dir_all(dir.join("usr/lib64")).unwrap();
        fs::write(dir.join("usr/lib64/libc.so.6"), "new").unwrap();

        assert!(layer_is_risky(&dir).unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn risky_when_the_kernel_changes() {
        let dir = scratch("risky-boot");
        fs::create_dir_all(dir.join("boot")).unwrap();
        fs::write(dir.join("boot/vmlinuz"), "new").unwrap();

        assert!(layer_is_risky(&dir).unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }
}
