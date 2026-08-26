use std::io;
use std::path::Path;

use rustix::fs::{XattrFlags, getxattr, setxattr};
use rustix::io::Errno;

/// OverlayFS's "lower layers fully shadowed here" marker. Setting
/// `trusted.*` xattrs needs real root (see copy_tree/squash's ignored tests).
const OPAQUE_XATTR: &str = "trusted.overlay.opaque";

pub fn is_opaque(path: &Path) -> io::Result<bool> {
    has_flag_xattr(path, OPAQUE_XATTR)
}

pub fn mark_opaque(path: &Path) -> io::Result<()> {
    set_flag_xattr(path, OPAQUE_XATTR)
}

/// Reads a boolean-style xattr (`"y"` means set); factored out so its logic
/// is testable against a permission-free namespace, unlike `trusted.*`.
fn has_flag_xattr(path: &Path, name: &str) -> io::Result<bool> {
    let mut buf = [0u8; 8];
    match getxattr(path, name, &mut buf[..]) {
        Ok(len) => Ok(&buf[..len] == b"y"),
        Err(Errno::NODATA) | Err(Errno::NOTSUP) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn set_flag_xattr(path: &Path, name: &str) -> io::Result<()> {
    setxattr(path, name, b"y", XattrFlags::empty()).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Same getxattr/setxattr pattern as `is_opaque`/`mark_opaque`, but
    /// against `user.*` so it runs without root.
    #[test]
    fn flag_xattr_roundtrips_and_defaults_to_absent() {
        let path = std::env::temp_dir().join(format!("layerfs-opaque-test-{}", std::process::id()));
        fs::write(&path, "").unwrap();

        assert!(!has_flag_xattr(&path, "user.layerfs-test-flag").unwrap());

        set_flag_xattr(&path, "user.layerfs-test-flag").unwrap();
        assert!(has_flag_xattr(&path, "user.layerfs-test-flag").unwrap());

        fs::remove_file(&path).unwrap();
    }
}
