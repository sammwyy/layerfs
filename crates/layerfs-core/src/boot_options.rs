use crate::checkpoint::Checkpoint;
use crate::error::CoreError;

/// Parsed `layerfs.*` kernel command-line parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootOptions {
    pub checkpoint: Checkpoint,
    pub head: bool,
    pub debug: bool,
    pub store: Option<String>,
    /// Btrfs subvolume the store lives in (`layerfs.subvol=<name>`), when
    /// it isn't the device's default subvolume.
    pub subvol: Option<String>,
    /// Encrypted device to unlock before `store` (`layerfs.luks=<spec>`).
    pub luks: Option<String>,
    pub luks_key: Option<String>,
    /// Retrofit-migration boot (`layerfs.migrate=1`): converts
    /// `migrate_source` into `store` instead of assembling a root.
    pub migrate: bool,
    pub migrate_source: Option<String>,
    /// Adapter names to activate for this boot (`layerfs.integrations=dnf,apt`).
    pub integrations: Vec<String>,
}

impl Default for BootOptions {
    fn default() -> Self {
        Self {
            checkpoint: Checkpoint::Normal,
            head: true,
            debug: false,
            store: None,
            subvol: None,
            luks: None,
            luks_key: None,
            migrate: false,
            migrate_source: None,
            integrations: Vec::new(),
        }
    }
}

impl BootOptions {
    /// Parses options out of a raw `/proc/cmdline` string. Unknown tokens
    /// are ignored; an invalid `layerfs.*` value is rejected outright.
    pub fn parse(cmdline: &str) -> Result<Self, CoreError> {
        let mut opts = BootOptions::default();

        for token in cmdline.split_whitespace() {
            let Some((key, value)) = token.split_once('=') else {
                continue;
            };

            match key {
                "layerfs.checkpoint" => opts.checkpoint = value.parse()?,
                "layerfs.head" => opts.head = parse_bool(key, value)?,
                "layerfs.debug" => opts.debug = parse_bool(key, value)?,
                "layerfs.store" => opts.store = Some(value.to_string()),
                "layerfs.subvol" => opts.subvol = Some(value.to_string()),
                "layerfs.luks" => opts.luks = Some(value.to_string()),
                "layerfs.luks_key" => opts.luks_key = Some(value.to_string()),
                "layerfs.migrate" => opts.migrate = parse_bool(key, value)?,
                "layerfs.migrate_source" => opts.migrate_source = Some(value.to_string()),
                "layerfs.integrations" => {
                    opts.integrations = value
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect()
                }
                _ => {}
            }
        }

        Ok(opts)
    }
}

fn parse_bool(key: &str, value: &str) -> Result<bool, CoreError> {
    match value {
        "on" | "1" => Ok(true),
        "off" | "0" => Ok(false),
        other => Err(CoreError::InvalidOption {
            key: key.to_string(),
            value: other.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_normal_with_head_on() {
        let opts = BootOptions::parse("root=/dev/sda1 quiet").unwrap();
        assert_eq!(opts.checkpoint, Checkpoint::Normal);
        assert!(opts.head);
    }

    #[test]
    fn parses_rollback_combination() {
        let opts = BootOptions::parse("layerfs.checkpoint=safe layerfs.head=off").unwrap();
        assert_eq!(opts.checkpoint, Checkpoint::Safe);
        assert!(!opts.head);
    }

    #[test]
    fn rejects_invalid_checkpoint() {
        assert!(BootOptions::parse("layerfs.checkpoint=bogus").is_err());
    }

    #[test]
    fn parses_integrations_list() {
        let opts = BootOptions::parse("layerfs.integrations=dnf,apt").unwrap();
        assert_eq!(opts.integrations, vec!["dnf", "apt"]);
    }

    #[test]
    fn defaults_to_no_integrations() {
        assert!(BootOptions::parse("").unwrap().integrations.is_empty());
    }

    #[test]
    fn parses_subvol() {
        let opts = BootOptions::parse("layerfs.store=UUID=abcd layerfs.subvol=layerfs").unwrap();
        assert_eq!(opts.subvol.as_deref(), Some("layerfs"));
    }

    #[test]
    fn defaults_to_no_subvol() {
        assert!(BootOptions::parse("").unwrap().subvol.is_none());
    }

    #[test]
    fn parses_luks_and_luks_key() {
        let opts = BootOptions::parse("layerfs.luks=UUID=abcd layerfs.luks_key=/key").unwrap();
        assert_eq!(opts.luks.as_deref(), Some("UUID=abcd"));
        assert_eq!(opts.luks_key.as_deref(), Some("/key"));
    }

    #[test]
    fn defaults_to_no_luks() {
        let opts = BootOptions::parse("").unwrap();
        assert!(opts.luks.is_none());
        assert!(opts.luks_key.is_none());
    }

    #[test]
    fn parses_migrate_and_migrate_source() {
        let opts = BootOptions::parse("layerfs.migrate=1 layerfs.migrate_source=UUID=old").unwrap();
        assert!(opts.migrate);
        assert_eq!(opts.migrate_source.as_deref(), Some("UUID=old"));
    }

    #[test]
    fn defaults_to_no_migration() {
        let opts = BootOptions::parse("").unwrap();
        assert!(!opts.migrate);
        assert!(opts.migrate_source.is_none());
    }
}
