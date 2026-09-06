use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioOptimization {
    #[default]
    Latency,
    Capacity,
}

impl AudioOptimization {
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
