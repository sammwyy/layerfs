//! Resolves `UUID=`/`LABEL=` device specs by reading each block device's
//! Btrfs superblock directly, since `rdinit=` skips dracut's udev entirely.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

const SUPER_INFO_OFFSET: u64 = 0x10000;
const SUPER_INFO_SIZE: usize = 4096;
const MAGIC_OFFSET: usize = 64;
const MAGIC: &[u8; 8] = b"_BHRfS_M";
const FSID_OFFSET: usize = 32;
const FSID_LEN: usize = 16;
const LABEL_OFFSET: usize = 299;
const LABEL_LEN: usize = 256;

pub fn find_by_uuid(uuid: &str) -> Option<PathBuf> {
    let uuid = uuid.to_ascii_lowercase();
    block_devices()
        .into_iter()
        .find(|dev| superblock_of(dev).is_some_and(|sb| sb.uuid() == uuid))
}

pub fn find_by_label(label: &str) -> Option<PathBuf> {
    block_devices()
        .into_iter()
        .find(|dev| superblock_of(dev).is_some_and(|sb| sb.label() == label))
}

struct Superblock([u8; SUPER_INFO_SIZE]);

impl Superblock {
    fn uuid(&self) -> String {
        format_uuid(&self.0[FSID_OFFSET..FSID_OFFSET + FSID_LEN])
    }

    fn label(&self) -> String {
        let raw = &self.0[LABEL_OFFSET..LABEL_OFFSET + LABEL_LEN];
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        String::from_utf8_lossy(&raw[..end]).into_owned()
    }
}

fn superblock_of(device: &Path) -> Option<Superblock> {
    let mut file = File::open(device).ok()?;
    file.seek(SeekFrom::Start(SUPER_INFO_OFFSET)).ok()?;
    let mut buf = [0u8; SUPER_INFO_SIZE];
    file.read_exact(&mut buf).ok()?;

    (buf[MAGIC_OFFSET..MAGIC_OFFSET + MAGIC.len()] == *MAGIC).then_some(Superblock(buf))
}

fn format_uuid(bytes: &[u8]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

fn block_devices() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir("/dev") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_block_device()))
        .map(|entry| entry.path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_uuid_bytes_as_a_standard_uuid_string() {
        let bytes = [
            0x54, 0xce, 0x21, 0xb9, 0x65, 0xe4, 0x4a, 0x2a, 0xaf, 0xa5, 0x41, 0x21, 0x77, 0x50,
            0xd0, 0x0b,
        ];
        assert_eq!(format_uuid(&bytes), "54ce21b9-65e4-4a2a-afa5-41217750d00b");
    }

    #[test]
    fn rejects_a_non_btrfs_file_as_no_superblock() {
        let path = std::env::temp_dir().join(format!("layerfs-not-btrfs-{}", std::process::id()));
        std::fs::write(&path, vec![0u8; SUPER_INFO_SIZE * 2]).unwrap();

        assert!(superblock_of(&path).is_none());

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "requires LAYERFS_BTRFS_STORE_DEVICE and read access to it"]
    fn finds_the_real_device_by_its_real_uuid_and_label() {
        let source = std::env::var("LAYERFS_BTRFS_STORE_DEVICE").unwrap();
        let uuid = std::env::var("LAYERFS_BTRFS_STORE_UUID").unwrap();
        let label = std::env::var("LAYERFS_BTRFS_STORE_LABEL").unwrap();

        let expected = std::fs::canonicalize(&source).unwrap();
        assert_eq!(find_by_uuid(&uuid), Some(expected.clone()));
        assert_eq!(find_by_label(&label), Some(expected));
    }
}
