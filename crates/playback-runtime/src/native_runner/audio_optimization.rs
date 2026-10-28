use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioOptimization {
    #[default]
    Latency,
    Capacity,
}

impl AudioOptimization {
    /// Lead time between a pulse and its sequenced output: Latency starts events inside the next
    /// rendered block, Capacity renders one quantum further ahead.
    pub const fn sequenced_output_delay(self) -> Duration {
        match self {
            Self::Latency => Duration::from_millis(8),
            Self::Capacity => Duration::from_millis(11),
        }
    }

    pub(super) const fn is_supported(self, capacity_available: bool) -> bool {
        matches!(self, Self::Latency) || capacity_available
    }

    pub(super) fn from_wire_name(value: &str) -> Option<Self> {
        match value {
            "latency" => Some(Self::Latency),
            "capacity" => Some(Self::Capacity),
            _ => None,
        }
    }
}
