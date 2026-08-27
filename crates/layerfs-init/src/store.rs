use std::path::{Path, PathBuf};

pub fn locate(explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(store) = explicit {
        return require_store(PathBuf::from(store));
    }

    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").map_err(|e| e.to_string())?;
    for mount in mountpoints(&mountinfo) {
        if mount.join("base").is_dir() {
            return Ok(mount);
        }
    }

    Err("no mounted LayerFS store found; pass layerfs.store=<path>".to_string())
}

fn require_store(path: PathBuf) -> Result<PathBuf, String> {
    if path.join("base").is_dir() {
        Ok(path)
    } else {
        Err(format!("{} is not a LayerFS store", path.display()))
    }
}

fn mountpoints(mountinfo: &str) -> impl Iterator<Item = PathBuf> + '_ {
    mountinfo.lines().filter_map(|line| {
        line.split(' ')
            .nth(4)
            .map(|mountpoint| Path::new(mountpoint).to_path_buf())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mountpoints() {
        let mounts =
            mountpoints("1 2 0:1 / / rw - rootfs rootfs rw\n3 1 0:2 / /store rw - tmpfs tmpfs rw")
                .collect::<Vec<_>>();
        assert_eq!(mounts, [PathBuf::from("/"), PathBuf::from("/store")]);
    }
}
