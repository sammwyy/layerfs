use std::path::Path;

use crate::error::CoreError;

/// Deterministically merges lower layer `a` and upper layer `b` into a
/// single layer at `dest`, producing the same view as `b > a`.
///
/// Must preserve OverlayFS whiteouts, opaque directories, xattrs, hardlinks,
/// symlinks, device nodes, and file capabilities. Not implemented as a
/// naive recursive copy; see milestone 6 in the design notes.
pub fn squash(a: &Path, b: &Path, dest: &Path) -> Result<(), CoreError> {
    let _ = (a, b, dest);
    Err(CoreError::NotImplemented("squash"))
}
