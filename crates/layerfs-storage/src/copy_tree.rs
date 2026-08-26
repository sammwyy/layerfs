use std::fs;
use std::io;
use std::path::Path;

use crate::opaque::{is_opaque, mark_opaque};
use crate::whiteout::{is_whiteout, write_whiteout};

/// Recursively copies `src` into `dest` (must not exist): dirs (preserving
/// opacity), files, symlinks, whiteouts — no other xattrs/ACLs/hardlinks.
pub fn copy_tree(src: &Path, dest: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(src)?;

    if metadata.is_dir() {
        fs::create_dir(dest)?;
        if is_opaque(src)? {
            mark_opaque(dest)?;
        }
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_tree(&entry.path(), &dest.join(entry.file_name()))?;
        }
    } else if metadata.file_type().is_symlink() {
        let target = fs::read_link(src)?;
        std::os::unix::fs::symlink(target, dest)?;
    } else if is_whiteout(&metadata) {
        write_whiteout(dest)?;
    } else if metadata.is_file() {
        fs::copy(src, dest)?;
    } else {
        return Err(io::Error::other(format!(
            "unsupported file type at {}: DirectoryBackend only handles files, dirs, symlinks, and whiteouts",
            src.display()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("layerfs-copy-tree-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn copies_files_dirs_and_symlinks() {
        let src = scratch("src");
        let dest = scratch("dest");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("sub/file.txt"), "hello").unwrap();
        std::os::unix::fs::symlink("file.txt", src.join("sub/link")).unwrap();

        copy_tree(&src, &dest).unwrap();

        assert_eq!(
            fs::read_to_string(dest.join("sub/file.txt")).unwrap(),
            "hello"
        );
        assert_eq!(
            fs::read_link(dest.join("sub/link")).unwrap(),
            std::path::Path::new("file.txt")
        );

        fs::remove_dir_all(&src).unwrap();
        fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    fn copies_whiteouts_as_whiteouts() {
        let src = scratch("wh-src");
        let dest = scratch("wh-dest");
        fs::create_dir_all(&src).unwrap();
        write_whiteout(&src.join("deleted")).unwrap();

        copy_tree(&src, &dest).unwrap();

        let meta = fs::symlink_metadata(dest.join("deleted")).unwrap();
        assert!(is_whiteout(&meta));

        fs::remove_dir_all(&src).unwrap();
        fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    #[ignore = "trusted.overlay.opaque requires real root (CAP_SYS_ADMIN in the init user namespace); see opaque.rs"]
    fn preserves_opaque_marker() {
        let src = scratch("opaque-src");
        let dest = scratch("opaque-dest");
        fs::create_dir_all(src.join("sub")).unwrap();
        mark_opaque(&src.join("sub")).unwrap();

        copy_tree(&src, &dest).unwrap();

        assert!(is_opaque(&dest.join("sub")).unwrap());

        fs::remove_dir_all(&src).unwrap();
        fs::remove_dir_all(&dest).unwrap();
    }
}
