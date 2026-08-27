use std::path::{Path, PathBuf};
use std::process::Command;

const MAPPER_NAME: &str = "layerfs-crypt";

pub fn unlock(device: &Path, key_file: Option<&str>) -> Result<PathBuf, String> {
    let mut cmd = Command::new("cryptsetup");
    cmd.arg("luksOpen").arg(device).arg(MAPPER_NAME);
    if let Some(key) = key_file {
        cmd.arg("--key-file").arg(key);
    }

    let status = cmd.status().map_err(|e| format!("run cryptsetup: {e}"))?;
    if !status.success() {
        return Err(format!(
            "cryptsetup luksOpen {}: {status}",
            device.display()
        ));
    }

    Ok(PathBuf::from(format!("/dev/mapper/{MAPPER_NAME}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires LAYERFS_LUKS_DEVICE, LAYERFS_LUKS_KEY, and CAP_SYS_ADMIN"]
    fn unlocks_a_real_luks_device_with_a_keyfile() {
        let device = std::env::var("LAYERFS_LUKS_DEVICE").unwrap();
        let key = std::env::var("LAYERFS_LUKS_KEY").unwrap();

        let mapper = unlock(Path::new(&device), Some(&key)).unwrap();

        assert_eq!(mapper, Path::new("/dev/mapper/layerfs-crypt"));
        assert!(mapper.exists());

        Command::new("cryptsetup")
            .args(["luksClose", MAPPER_NAME])
            .status()
            .unwrap();
    }
}
