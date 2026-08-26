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

/// MVP structural validation for a BASE (or any assembled-root candidate)
/// layer, per the design notes: this proves the layer is structurally
/// plausible, not that it will boot.
pub fn verify_base(base: &Path) -> Report {
    let exists = |rel: &str| base.join(rel).exists();

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
        let dir = std::env::temp_dir().join(format!("layerctl-verify-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(dir.join("usr")).unwrap();
        fs::create_dir_all(dir.join("etc")).unwrap();
        // no bin

        let report = verify_base(&dir);
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
            std::env::temp_dir().join(format!("layerctl-verify-test-ok-{}", std::process::id()));
        fs::create_dir_all(dir.join("usr/bin")).unwrap();
        fs::create_dir_all(dir.join("etc")).unwrap();

        let report = verify_base(&dir);
        assert!(report.passed());

        fs::remove_dir_all(&dir).unwrap();
    }
}
