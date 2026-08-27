use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;

use crate::copy_tree::copy_tree;
use crate::opaque::{is_opaque, mark_opaque};
use crate::whiteout::{is_whiteout, write_whiteout};

/// Merges lower `a` and upper `b` into `dest` as mounting `b` over `a`
/// would show it. Whiteouts/opaque markers from `b` are kept, not resolved
/// away, since `dest` still needs to hide whatever ends up below it.
pub fn squash(a: &Path, b: &Path, dest: &Path) -> io::Result<()> {
    if !a.is_dir() {
        return Err(io::Error::other(format!(
            "squash: {} is not a directory",
            a.display()
        )));
    }
    if !b.is_dir() {
        return Err(io::Error::other(format!(
            "squash: {} is not a directory",
            b.display()
        )));
    }

    merge_dir(Some(a), Some(b), dest)
}

fn merge_dir(a_dir: Option<&Path>, b_dir: Option<&Path>, dest_dir: &Path) -> io::Result<()> {
    fs::create_dir(dest_dir)?;

    let b_opaque = match b_dir {
        Some(p) => is_opaque(p)?,
        None => false,
    };
    if b_opaque {
        mark_opaque(dest_dir)?;
    }

    // Opaque b_dir shadows a_dir entirely, even names b_dir doesn't mention.
    let effective_a_dir = if b_opaque { None } else { a_dir };

    let a_names = list_names(effective_a_dir)?;
    let b_names = list_names(b_dir)?;
    let all_names: BTreeSet<&OsString> = a_names.union(&b_names).collect();

    for name in all_names {
        let a_path = effective_a_dir
            .filter(|_| a_names.contains(name))
            .map(|p| p.join(name));
        let b_path = b_dir
            .filter(|_| b_names.contains(name))
            .map(|p| p.join(name));
        let dest_path = dest_dir.join(name);

        if let Some(bp) = &b_path {
            merge_from_b(a_path.as_deref(), bp, &dest_path)?;
        } else if let Some(ap) = &a_path {
            copy_tree(ap, &dest_path)?;
        }
    }

    Ok(())
}

fn merge_from_b(a_path: Option<&Path>, b_path: &Path, dest_path: &Path) -> io::Result<()> {
    let b_meta = fs::symlink_metadata(b_path)?;

    if is_whiteout(&b_meta) {
        // a's version, if any, is superseded; the deletion must still show through.
        return write_whiteout(dest_path);
    }

    if b_meta.is_dir() {
        let a_is_dir = match a_path {
            Some(ap) => fs::symlink_metadata(ap)?.is_dir(),
            None => false,
        };

        if a_is_dir {
            return merge_dir(a_path, Some(b_path), dest_path);
        }
    }

    copy_tree(b_path, dest_path)
}

fn list_names(dir: Option<&Path>) -> io::Result<BTreeSet<OsString>> {
    let Some(dir) = dir else {
        return Ok(BTreeSet::new());
    };

    let mut names = BTreeSet::new();
    for entry in fs::read_dir(dir)? {
        names.insert(entry?.file_name());
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("layerfs-squash-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn upper_file_shadows_lower_file() {
        let root = scratch("shadow");
        let a = root.join("a");
        let b = root.join("b");
        let dest = root.join("dest");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("f.txt"), "lower").unwrap();
        fs::write(b.join("f.txt"), "upper").unwrap();

        squash(&a, &b, &dest).unwrap();

        assert_eq!(fs::read_to_string(dest.join("f.txt")).unwrap(), "upper");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn upper_whiteout_deletes_lower_file_and_persists() {
        let root = scratch("whiteout");
        let a = root.join("a");
        let b = root.join("b");
        let dest = root.join("dest");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("f.txt"), "lower").unwrap();
        write_whiteout(&b.join("f.txt")).unwrap();

        squash(&a, &b, &dest).unwrap();

        let meta = fs::symlink_metadata(dest.join("f.txt")).unwrap();
        assert!(
            is_whiteout(&meta),
            "whiteout must survive the squash to keep hiding BASE"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn lower_whiteout_not_touched_by_upper_persists() {
        let root = scratch("lower-whiteout");
        let a = root.join("a");
        let b = root.join("b");
        let dest = root.join("dest");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        write_whiteout(&a.join("f.txt")).unwrap();

        squash(&a, &b, &dest).unwrap();

        let meta = fs::symlink_metadata(dest.join("f.txt")).unwrap();
        assert!(is_whiteout(&meta));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn entries_only_in_lower_are_kept() {
        let root = scratch("lower-only");
        let a = root.join("a");
        let b = root.join("b");
        let dest = root.join("dest");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("only-a.txt"), "a").unwrap();

        squash(&a, &b, &dest).unwrap();

        assert_eq!(fs::read_to_string(dest.join("only-a.txt")).unwrap(), "a");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn matching_directories_merge_recursively() {
        let root = scratch("recursive-merge");
        let a = root.join("a");
        let b = root.join("b");
        let dest = root.join("dest");
        fs::create_dir_all(a.join("sub")).unwrap();
        fs::create_dir_all(b.join("sub")).unwrap();
        fs::write(a.join("sub/only-a.txt"), "a").unwrap();
        fs::write(b.join("sub/only-b.txt"), "b").unwrap();

        squash(&a, &b, &dest).unwrap();

        assert_eq!(
            fs::read_to_string(dest.join("sub/only-a.txt")).unwrap(),
            "a"
        );
        assert_eq!(
            fs::read_to_string(dest.join("sub/only-b.txt")).unwrap(),
            "b"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    #[ignore = "trusted.overlay.opaque requires real root (CAP_SYS_ADMIN in the init user namespace); see opaque.rs"]
    fn opaque_upper_directory_hides_lower_subtree_and_stays_opaque() {
        let root = scratch("opaque");
        let a = root.join("a");
        let b = root.join("b");
        let dest = root.join("dest");
        fs::create_dir_all(a.join("sub")).unwrap();
        fs::write(a.join("sub/hidden.txt"), "should not appear").unwrap();
        fs::create_dir_all(b.join("sub")).unwrap();
        fs::write(b.join("sub/visible.txt"), "should appear").unwrap();
        mark_opaque(&b.join("sub")).unwrap();

        squash(&a, &b, &dest).unwrap();

        assert!(!dest.join("sub/hidden.txt").exists());
        assert!(dest.join("sub/visible.txt").exists());
        assert!(
            is_opaque(&dest.join("sub")).unwrap(),
            "opacity must survive to keep hiding BASE below dest"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn upper_type_change_wins_outright() {
        let root = scratch("type-change");
        let a = root.join("a");
        let b = root.join("b");
        let dest = root.join("dest");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("thing"), "was a file").unwrap();
        fs::create_dir_all(b.join("thing")).unwrap();
        fs::write(b.join("thing/inside.txt"), "now a dir").unwrap();

        squash(&a, &b, &dest).unwrap();

        assert!(dest.join("thing").is_dir());
        assert_eq!(
            fs::read_to_string(dest.join("thing/inside.txt")).unwrap(),
            "now a dir"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn upper_type_change_dir_to_file_wins_outright() {
        let root = scratch("type-change-reverse");
        let a = root.join("a");
        let b = root.join("b");
        let dest = root.join("dest");
        fs::create_dir_all(a.join("thing")).unwrap();
        fs::write(a.join("thing/inside.txt"), "was a dir").unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(b.join("thing"), "now a file").unwrap();

        squash(&a, &b, &dest).unwrap();

        assert!(dest.join("thing").is_file());
        assert_eq!(
            fs::read_to_string(dest.join("thing")).unwrap(),
            "now a file"
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
