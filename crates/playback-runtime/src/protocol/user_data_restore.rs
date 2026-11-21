use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeUserDataRestorePhase {
    Restoring,
    Succeeded,
    Failed,
    /// Refused before it started: an earlier restore left staging that only a restart finishes.
    EarlierRestoreUnfinished,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeUserDataRestoreStatus {
    pub phase: RuntimeUserDataRestorePhase,
}
