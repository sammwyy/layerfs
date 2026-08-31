//! Writes the provider-neutral boot mount ownership handoff.

use std::io;
use std::path::{Path, PathBuf};

const CLAIMS_PATH: &str = "etc/diskd/boot-mounts.toml";

/// Write mount paths that later storage policy must not manage.
pub fn write(root: &Path, paths: impl IntoIterator<Item = PathBuf>) -> io::Result<()> {
    let path = root.join(CLAIMS_PATH);
    let mut document = String::from("version = 1\n");
    for mountpoint in paths {
        document.push_str("\n[[mount]]\npath = ");
        document.push_str(&toml_string(&mountpoint)?);
        document.push('\n');
    }
    std::fs::create_dir_all(path.parent().expect("claims path has a parent"))?;
    std::fs::write(path, document)
}

fn toml_string(path: &Path) -> io::Result<String> {
    let value = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "mount path is not UTF-8"))?;
    if value.contains(['\n', '\r', '\0', '"', '\\']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mount path cannot be represented safely",
        ));
    }
    Ok(format!("\"{value}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_bounded_claim_document() {
        let root = std::env::temp_dir().join(format!("layerfs-claims-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        write(&root, [PathBuf::from("/"), PathBuf::from("/home")]).unwrap();
        let document = std::fs::read_to_string(root.join(CLAIMS_PATH)).unwrap();
        assert_eq!(
            document,
            "version = 1\n\n[[mount]]\npath = \"/\"\n\n[[mount]]\npath = \"/home\"\n"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
