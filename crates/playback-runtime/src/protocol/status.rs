use super::RuntimeErrorMetadata;
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

impl RuntimeTransportState {
    /// Whether going from `self` to `next` releases held notes and silences internal audio:
    /// leaving Playing, or stopping from Paused.
    pub(crate) fn change_silences_audio(&self, next: &Self) -> bool {
        (*self == Self::Playing && *next != Self::Playing)
            || (*self == Self::Paused && *next == Self::Stopped)
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RuntimeErrorMetadata>,
}
