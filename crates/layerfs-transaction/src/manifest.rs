use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

use crate::state::TransactionRecord;

const MANIFEST_FILENAME: &str = "manifest.log";

pub fn append(store_root: &Path, record: &TransactionRecord) -> io::Result<()> {
    let line = serde_json::to_string(record).map_err(io::Error::other)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(store_root.join(MANIFEST_FILENAME))?;
    writeln!(file, "{line}")
}

pub fn read(store_root: &Path) -> io::Result<Vec<TransactionRecord>> {
    let contents = match std::fs::read_to_string(store_root.join(MANIFEST_FILENAME)) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    contents
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).map_err(io::Error::other))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::TransactionState;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("layerfs-manifest-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn record(program: &str, args: &[&str]) -> TransactionRecord {
        TransactionRecord {
            id: "txn-1".to_string(),
            kind: "system".to_string(),
            started_at_unix: 0,
            adapter: "dnf".to_string(),
            state: TransactionState::Committed,
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn missing_manifest_reads_as_empty() {
        let dir = scratch("missing");
        assert_eq!(read(&dir).unwrap(), Vec::new());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn append_then_read_round_trips_in_order() {
        let dir = scratch("roundtrip");
        append(&dir, &record("dnf", &["install", "vim"])).unwrap();
        append(&dir, &record("dnf", &["install", "curl"])).unwrap();

        let entries = read(&dir).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].args, vec!["install", "vim"]);
        assert_eq!(entries[1].args, vec!["install", "curl"]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
