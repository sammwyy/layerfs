use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    Preparing,
    Running,
    Validating,
    Committing,
    Committed,
    Failed,
}

/// Record describing an in-progress or completed system transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub id: String,
    pub kind: String,
    pub started_at_unix: u64,
    pub adapter: String,
    pub state: TransactionState,
}
