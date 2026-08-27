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
            integrations: Vec::new(),
        }
    }
}

impl BootOptions {
    /// Parses options out of a raw `/proc/cmdline` string.
    ///
    /// Unknown non-`layerfs.*` tokens are ignored. An invalid `layerfs.*`
    /// value is rejected rather than silently falling back, per the
    /// fail-safe boot policy.
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
}
