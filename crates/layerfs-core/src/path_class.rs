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

/// Root-relative names of the virtual filesystem mount points a system
/// transaction binds a real package manager's scriptlets onto. `BASE` must
/// always carry these as plain empty directories (real distros do, via
/// packages like `filesystem`): binding onto a path that already resolves
/// through a lower layer is a pure mount operation with no copy-up, while
/// binding onto a path that has to be `mkdir`'d first copies an otherwise
/// empty directory into `HEAD.next`'s upper layer — which then reads as a
/// change outside `usr`/`opt` and permanently blocks hot-apply.
pub const VIRTUAL_MOUNTS: &[&str] = &["proc", "sys", "dev", "run"];

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
