use std::fmt;
use std::str::FromStr;

use crate::error::CoreError;

/// One of the four hardcoded LayerFS boot checkpoints.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Checkpoint {
    Base = 0,
    System = 1,
    Safe = 2,
    Normal = 3,
}

impl Checkpoint {
    pub const ALL: [Checkpoint; 4] = [
        Checkpoint::Base,
        Checkpoint::System,
        Checkpoint::Safe,
        Checkpoint::Normal,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Checkpoint::Base => "base",
            Checkpoint::System => "system",
            Checkpoint::Safe => "safe",
            Checkpoint::Normal => "normal",
        }
    }

    /// Whether this checkpoint includes the writable OVERRIDE layer.
    pub fn includes_override(self) -> bool {
        matches!(self, Checkpoint::Normal)
    }

    /// Whether this checkpoint mounts persistent DATA.
    pub fn includes_data(self) -> bool {
        matches!(self, Checkpoint::Safe | Checkpoint::Normal)
    }

    /// Whether this checkpoint includes UPDATE/UPDATE_HEAD at all.
    pub fn includes_update(self) -> bool {
        !matches!(self, Checkpoint::Base)
    }
}

impl fmt::Display for Checkpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Checkpoint {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "0" | "base" => Ok(Checkpoint::Base),
            "1" | "system" => Ok(Checkpoint::System),
            "2" | "safe" => Ok(Checkpoint::Safe),
            "3" | "normal" => Ok(Checkpoint::Normal),
            other => Err(CoreError::InvalidCheckpoint(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_and_named_forms() {
        assert_eq!("0".parse::<Checkpoint>().unwrap(), Checkpoint::Base);
        assert_eq!("normal".parse::<Checkpoint>().unwrap(), Checkpoint::Normal);
    }

    #[test]
    fn rejects_unknown_values() {
        assert!("4".parse::<Checkpoint>().is_err());
        assert!("nope".parse::<Checkpoint>().is_err());
    }

    #[test]
    fn only_normal_has_override() {
        for cp in Checkpoint::ALL {
            assert_eq!(cp.includes_override(), cp == Checkpoint::Normal);
        }
    }
}
