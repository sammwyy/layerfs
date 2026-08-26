use serde::{Deserialize, Serialize};

/// Durable identifiers of the currently active UPDATE/UPDATE_HEAD pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateState {
    pub update: String,
    pub update_head: String,
}

/// On-disk LayerFS state metadata (`state.json`).
///
/// Written via write-temp/fsync/rename so a crash never leaves a partial
/// file in place; see the storage backend for the commit implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub version: u32,
    pub base: String,
    pub update: Option<UpdateState>,
    pub r#override: Option<String>,
    pub transaction: Option<String>,
}

impl State {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn new(base: impl Into<String>) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            base: base.into(),
            update: None,
            r#override: None,
            transaction: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let mut state = State::new("base");
        state.update = Some(UpdateState {
            update: "update-42".into(),
            update_head: "head-43".into(),
        });

        let json = serde_json::to_string(&state).unwrap();
        let parsed: State = serde_json::from_str(&json).unwrap();
        assert_eq!(state, parsed);
    }
}
