use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use rustix::fs::{CWD, FileType, Mode, mknodat};

/// An OverlayFS whiteout: a character device with major/minor 0,0,
/// recording that a path is deleted relative to lower layers.
pub fn is_whiteout(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_char_device() && metadata.rdev() == 0
}

pub fn write_whiteout(path: &Path) -> io::Result<()> {
    mknodat(CWD, path, FileType::CharacterDevice, Mode::empty(), 0).map_err(Into::into)
}
