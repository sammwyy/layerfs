/// Classification of a root path for persistence/inspection purposes.
///
/// `/var/lib` is intentionally not blanket-classified as `Data`; package
/// managers keep consistency-sensitive databases there (rpm, dpkg) that
/// must move in lockstep with the system layers, not persist independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathClass {
    System,
    Data,
}

/// Root-relative names of the persistent DATA mount points, without a
/// leading slash. The same list drives both `classify()` and the DATA
/// bind-mount plan built during root assembly.
pub const DATA_MOUNTS: &[&str] = &["home", "root", "srv"];

pub fn classify(path: &str) -> PathClass {
    let is_data = DATA_MOUNTS.iter().any(|name| {
        let prefix = format!("/{name}");
        path == prefix || path.starts_with(&format!("{prefix}/"))
    });

    if is_data {
        PathClass::Data
    } else {
        PathClass::System
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_home_as_data() {
        assert_eq!(classify("/home/user/file"), PathClass::Data);
    }

    #[test]
    fn classifies_var_lib_as_system() {
        assert_eq!(classify("/var/lib/rpm"), PathClass::System);
    }
}
