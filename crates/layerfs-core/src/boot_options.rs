use crate::checkpoint::Checkpoint;
use crate::error::CoreError;

/// Parsed `layerfs.*` kernel command-line parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootOptions {
    pub checkpoint: Checkpoint,
    pub head: bool,
    pub debug: bool,
    pub store: Option<String>,
}

impl Default for BootOptions {
    fn default() -> Self {
        Self {
            checkpoint: Checkpoint::Normal,
            head: true,
            debug: false,
            store: None,
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
}
