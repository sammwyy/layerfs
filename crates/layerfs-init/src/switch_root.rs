use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use rustix::mount::mount_move;
use rustix::process::chroot;

pub fn switch_root(new_root: &Path, init: &Path) -> io::Result<()> {
    std::env::set_current_dir(new_root)
        .map_err(|error| io::Error::other(format!("chdir {}: {error}", new_root.display())))?;

    let mounts = ["dev", "proc", "sys", "run"];
    let present = mounts.map(|mount| is_mountpoint(&Path::new("/").join(mount)));

    for (mount, present) in mounts.into_iter().zip(present) {
        if !present? {
            continue;
        }
        let from = Path::new("/").join(mount);

        let to = new_root.join(mount);
        std::fs::create_dir_all(&to)
            .map_err(|error| io::Error::other(format!("create {}: {error}", to.display())))?;
        mount_move(&from, &to)
            .map_err(io::Error::from)
            .map_err(|error| io::Error::other(format!("move {}: {error}", from.display())))?;
    }

    mount_move(new_root, Path::new("/"))
        .map_err(io::Error::from)
        .map_err(|error| io::Error::other(format!("move {} to /: {error}", new_root.display())))?;
    chroot(".")
        .map_err(io::Error::from)
        .map_err(|error| io::Error::other(format!("chroot: {error}")))?;
    std::env::set_current_dir("/")
        .map_err(|error| io::Error::other(format!("chdir /: {error}")))?;

    Err(io::Error::other(format!(
        "exec {}: {}",
        init.display(),
        Command::new(init).exec()
    )))
}

fn is_mountpoint(path: &Path) -> io::Result<bool> {
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")?;
    let path = path.to_string_lossy();
    Ok(mountinfo
        .lines()
        .any(|line| line.split(' ').nth(4) == Some(path.as_ref())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_mountinfo_mountpoints() {
        let path = Path::new("/");
        assert!(is_mountpoint(path).unwrap());
    }
}
