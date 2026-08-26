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

const DATA_PREFIXES: &[&str] = &["/home", "/root", "/srv"];

pub fn classify(path: &str) -> PathClass {
    if DATA_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
    {
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
