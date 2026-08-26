use std::ffi::CString;
use std::fs;
use std::io;
use std::path::Path;

use rustix::mount::{MountFlags, mount, mount_bind};

use layerfs_core::{DATA_MOUNTS, LayerStack};

/// Mounts a resolved layer stack at `target`.
///
/// A stack with a single read-only layer and no upper is bind-mounted
/// directly. Anything else is assembled as an OverlayFS mount; `work_dir`
/// is only touched (and must exist) when the stack has a writable upper.
///
/// Shared by `layerfs-init` (boot-time root assembly) and
/// `layerfs-transaction` (system transaction roots) — both need the exact
/// same mount mechanics, just different `LayerStack`s.
pub fn assemble(stack: &LayerStack, work_dir: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;

    let upper = stack.upper();
    let lowers = stack.lowers();

    if upper.is_none() && lowers.len() == 1 {
        mount_bind(&lowers[0].path, target)?;
        return Ok(());
    }

    let lowerdir = lowers
        .iter()
        .map(|l| l.path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(":");

    let mut options = format!("lowerdir={lowerdir}");
    if let Some(upper) = upper {
        fs::create_dir_all(work_dir)?;
        options.push_str(&format!(
            ",upperdir={},workdir={}",
            upper.path.display(),
            work_dir.display()
        ));
    }

    let options = CString::new(options).map_err(io::Error::other)?;
    mount(
        "overlay",
        target,
        "overlay",
        MountFlags::empty(),
        options.as_c_str(),
    )
    .map_err(io::Error::from)
}

/// Bind-mounts each present DATA subdirectory (`layerfs_core::DATA_MOUNTS`)
/// from `data_root` onto the matching path under the assembled `target`.
/// Missing subdirectories are skipped rather than created — DATA is
/// irreplaceable and LayerFS must not invent it.
pub fn mount_data(data_root: &Path, target: &Path) -> io::Result<()> {
    for name in DATA_MOUNTS {
        let source = data_root.join(name);
        if !source.is_dir() {
            continue;
        }

        let dest = target.join(name);
        fs::create_dir_all(&dest)?;
        mount_bind(&source, &dest)?;
    }

    Ok(())
}
