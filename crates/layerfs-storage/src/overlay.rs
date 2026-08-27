use std::ffi::CString;
use std::fs;
use std::io;
use std::path::Path;

use rustix::mount::{MountFlags, mount, mount_bind, mount_move};

use layerfs_core::{DATA_MOUNTS, Layer, LayerKind, LayerStack};

/// Mounts a resolved layer stack at `target`: bind-mount if it's a single
/// read-only layer, else an OverlayFS mount (`work_dir` only used then).
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

/// Layers `new_lowers` over a snapshot of `target`, then swaps it in via
/// `mount --move`. Caller must scope `target` to a subtree with no nested
/// mounts, or `mount --move` orphans them.
pub fn hot_apply(
    target: &Path,
    new_lowers: &[&Path],
    override_dir: &Path,
    work_dir: &Path,
    snapshot_dir: &Path,
    staging_dir: &Path,
) -> io::Result<()> {
    fs::create_dir_all(snapshot_dir)?;
    mount_bind(target, snapshot_dir)?;

    let mut stack = LayerStack::new();
    stack.push(Layer::new(
        LayerKind::Override,
        "override",
        override_dir,
        false,
    ));
    for (i, lower) in new_lowers.iter().enumerate() {
        stack.push(Layer::new(
            LayerKind::Update,
            format!("hot-{i}"),
            *lower,
            true,
        ));
    }
    stack.push(Layer::new(
        LayerKind::Base,
        "live-snapshot",
        snapshot_dir,
        true,
    ));

    assemble(&stack, work_dir, staging_dir)?;
    mount_move(staging_dir, target).map_err(io::Error::from)
}

/// Bind-mounts each present DATA subdirectory onto `target`; missing ones
/// are skipped, never created — DATA is irreplaceable.
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
