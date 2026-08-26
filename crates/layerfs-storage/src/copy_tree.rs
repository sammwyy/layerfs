use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use rustix::fs::{CWD, mknodat};
use rustix::io::Errno;

/// Recursively copies `src` into `dest`, which must not already exist.
///
/// Handles the object kinds a `DirectoryBackend` layer actually contains:
/// directories, regular files, symlinks, and OverlayFS whiteouts (0,0
/// character devices). Does not preserve xattrs, ACLs, hardlinks, or
/// opaque-directory markers — a known limitation of the generic backend,
/// same tradeoff as the rest of `DirectoryBackend` (see section 18).
pub fn copy_tree(src: &Path, dest: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(src)?;

    if metadata.is_dir() {
        fs::create_dir(dest)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_tree(&entry.path(), &dest.join(entry.file_name()))?;
        }
    } else if metadata.file_type().is_symlink() {
        let target = fs::read_link(src)?;
        std::os::unix::fs::symlink(target, dest)?;
    } else if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
        mknodat(
            CWD,
            dest,
            rustix::fs::FileType::CharacterDevice,
            rustix::fs::Mode::empty(),
            0,
        )
        .map_err(errno_to_io)?;
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

fn errno_to_io(e: Errno) -> io::Error {
    e.into()
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
        rustix::fs::mknodat(
            CWD,
            src.join("deleted"),
            rustix::fs::FileType::CharacterDevice,
            rustix::fs::Mode::empty(),
            0,
        )
        .unwrap();

        copy_tree(&src, &dest).unwrap();

        let meta = fs::symlink_metadata(dest.join("deleted")).unwrap();
        assert!(meta.file_type().is_char_device());
        assert_eq!(meta.rdev(), 0);

        fs::remove_dir_all(&src).unwrap();
        fs::remove_dir_all(&dest).unwrap();
    }
}
