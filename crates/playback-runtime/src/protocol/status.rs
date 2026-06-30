use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncSource {
    Internal,
    External,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTransportState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStatusState {
    Idle,
    Running,
    Paused,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub state: RuntimeStatusState,
    pub transport: RuntimeTransportState,
    #[serde(rename = "currentPpqnPulse")]
    pub current_ppqn_pulse: u64,
    #[serde(rename = "pendingResync")]
    pub pending_resync: bool,
    #[serde(rename = "syncSource")]
    pub sync_source: SyncSource,
    #[serde(default)]
    pub message: Option<String>,
}
