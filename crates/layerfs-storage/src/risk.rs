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

/// Top-level directories a scoped hot-apply is allowed to touch — ones
/// that don't normally have their own mounts nested under them, so
/// replacing just that subtree can't orphan a submount the way replacing
/// `/` itself could (e.g. `/proc`, `/home`, `/boot`).
const HOT_APPLICABLE_SCOPES: &[&str] = &["usr", "opt"];

/// The top-level names touched across `layers`, if every one of them is in
/// `HOT_APPLICABLE_SCOPES`; `None` if any isn't (e.g. `/etc`), meaning a
/// scoped hot-apply isn't safe and the whole update needs a reboot.
pub fn hot_applicable_scopes(layers: &[&Path]) -> io::Result<Option<Vec<String>>> {
    let mut scopes = Vec::new();
    for layer in layers {
        for entry in fs::read_dir(layer)? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if !HOT_APPLICABLE_SCOPES.contains(&name.as_str()) {
                return Ok(None);
            }
            if !scopes.contains(&name) {
                scopes.push(name);
            }
        }
    }
    Ok(Some(scopes))
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

    #[test]
    fn usr_only_change_is_hot_applicable() {
        let dir = scratch("scope-usr");
        fs::create_dir_all(dir.join("usr/bin")).unwrap();

        assert_eq!(
            hot_applicable_scopes(&[&dir]).unwrap(),
            Some(vec!["usr".to_string()])
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn etc_change_is_not_hot_applicable() {
        let dir = scratch("scope-etc");
        fs::create_dir_all(dir.join("etc")).unwrap();

        assert_eq!(hot_applicable_scopes(&[&dir]).unwrap(), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn scopes_are_deduplicated_across_layers() {
        let a = scratch("scope-dup-a");
        let b = scratch("scope-dup-b");
        fs::create_dir_all(a.join("usr")).unwrap();
        fs::create_dir_all(b.join("usr")).unwrap();
        fs::create_dir_all(b.join("opt")).unwrap();

        let mut scopes = hot_applicable_scopes(&[&a, &b]).unwrap().unwrap();
        scopes.sort();
        assert_eq!(scopes, vec!["opt".to_string(), "usr".to_string()]);

        fs::remove_dir_all(&a).unwrap();
        fs::remove_dir_all(&b).unwrap();
    }
}
