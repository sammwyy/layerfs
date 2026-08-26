use std::path::Path;

pub struct Check {
    pub description: String,
    pub ok: bool,
}

pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.ok)
    }
}

/// MVP structural validation for an assembled (or candidate) root: proves
/// it is structurally plausible, not that it will boot — rollback exists
/// precisely because some failures can only be discovered during boot
/// (section 29).
pub fn verify_root(root: &Path) -> Report {
    let exists = |rel: &str| root.join(rel).exists();

    let checks = vec![
        Check {
            description: "/usr exists".to_string(),
            ok: exists("usr"),
        },
        Check {
            description: "/etc exists".to_string(),
            ok: exists("etc"),
        },
        Check {
            description: "/bin or /usr/bin exists".to_string(),
            ok: exists("bin") || exists("usr/bin"),
        },
    ];

    Report { checks }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn flags_missing_directories() {
        let dir =
            std::env::temp_dir().join(format!("layerfs-validate-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(dir.join("usr")).unwrap();
        fs::create_dir_all(dir.join("etc")).unwrap();
        // no bin

        let report = verify_root(&dir);
        assert!(!report.passed());
        assert!(
            report
                .checks
                .iter()
                .any(|c| !c.ok && c.description.contains("bin"))
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn passes_a_plausible_root() {
        let dir =
            std::env::temp_dir().join(format!("layerfs-validate-test-ok-{}", std::process::id()));
        fs::create_dir_all(dir.join("usr/bin")).unwrap();
        fs::create_dir_all(dir.join("etc")).unwrap();

        let report = verify_root(&dir);
        assert!(report.passed());

        fs::remove_dir_all(&dir).unwrap();
    }
}
