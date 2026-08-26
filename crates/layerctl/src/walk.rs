use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// Regular file, directory, or symlink physically present in the layer.
    Present,
    /// OverlayFS whiteout: a character device with major/minor 0,0,
    /// recording that this path is deleted relative to lower layers.
    Whiteout,
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Path relative to the layer root.
    pub path: PathBuf,
    pub kind: EntryKind,
}

/// Recursively lists everything stored directly in a layer directory,
/// depth-first, sorted. Does not follow symlinks.
pub fn walk(layer_root: &Path) -> std::io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    walk_into(layer_root, Path::new(""), &mut entries)?;
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

fn walk_into(root: &Path, rel: &Path, out: &mut Vec<Entry>) -> std::io::Result<()> {
    for entry in fs::read_dir(root.join(rel))? {
        let entry = entry?;
        let rel_path = rel.join(entry.file_name());
        let metadata = entry.metadata()?;

        let kind = if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
            EntryKind::Whiteout
        } else {
            EntryKind::Present
        };

        out.push(Entry {
            path: rel_path.clone(),
            kind,
        });

        if metadata.is_dir() {
            walk_into(root, &rel_path, out)?;
        }
    }

    Ok(())
}
