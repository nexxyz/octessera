use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DspRuntimeConfig {
    #[serde(default = "default_worker_warning_threshold")]
    pub worker_warning_threshold: WorkerWarningThreshold,
    #[serde(default = "default_bus_idle_threshold")]
    pub bus_idle_threshold: BusIdleThreshold,
}

impl Default for DspRuntimeConfig {
    fn default() -> Self {
        Self {
            worker_warning_threshold: WorkerWarningThreshold::Percent85,
            bus_idle_threshold: BusIdleThreshold::Db120,
        }
    }
}

impl DspRuntimeConfig {
    pub fn from_value(value: &Value) -> Result<Self, String> {
        serde_json::from_value(value.clone())
            .map_err(|error| format!("invalid DSP runtime config: {error}"))
    }

    pub fn to_value(self) -> Result<Value, String> {
        serde_json::to_value(self)
            .map_err(|error| format!("DSP runtime config encode failed: {error}"))
    }
}

fn default_worker_warning_threshold() -> WorkerWarningThreshold {
    WorkerWarningThreshold::Percent85
}

fn default_bus_idle_threshold() -> BusIdleThreshold {
    BusIdleThreshold::Db120
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerWarningThreshold {
    #[serde(rename = "70")]
    Percent70,
    #[serde(rename = "75")]
    Percent75,
    #[serde(rename = "80")]
    Percent80,
    #[serde(rename = "85")]
    Percent85,
    #[serde(rename = "90")]
    Percent90,
    #[serde(rename = "95")]
    Percent95,
}

impl WorkerWarningThreshold {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Percent70 => "70",
            Self::Percent75 => "75",
            Self::Percent80 => "80",
            Self::Percent85 => "85",
            Self::Percent90 => "90",
            Self::Percent95 => "95",
        }
    }

    pub const fn percent(self) -> u8 {
        match self {
            Self::Percent70 => 70,
            Self::Percent75 => 75,
            Self::Percent80 => 80,
            Self::Percent85 => 85,
            Self::Percent90 => 90,
            Self::Percent95 => 95,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "70" => Some(Self::Percent70),
            "75" => Some(Self::Percent75),
            "80" => Some(Self::Percent80),
            "85" => Some(Self::Percent85),
            "90" => Some(Self::Percent90),
            "95" => Some(Self::Percent95),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BusIdleThreshold {
    #[serde(rename = "exact")]
    Exact,
    #[serde(rename = "-140")]
    Db140,
    #[serde(rename = "-120")]
    Db120,
    #[serde(rename = "-100")]
    Db100,
    #[serde(rename = "-80")]
    Db80,
}

impl BusIdleThreshold {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Db140 => "-140",
            Self::Db120 => "-120",
            Self::Db100 => "-100",
            Self::Db80 => "-80",
        }
    }

    pub const fn amplitude(self) -> f32 {
        match self {
            Self::Exact => 0.0,
            Self::Db140 => 0.0000001,
            Self::Db120 => 0.000001,
            Self::Db100 => 0.00001,
            Self::Db80 => 0.0001,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "exact" => Some(Self::Exact),
            "-140" => Some(Self::Db140),
            "-120" => Some(Self::Db120),
            "-100" => Some(Self::Db100),
            "-80" => Some(Self::Db80),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dsp_runtime_config_round_trips_and_defaults() {
        assert_eq!(
            DspRuntimeConfig::from_value(&json!({})).unwrap(),
            DspRuntimeConfig::default()
        );
        assert_eq!(
            DspRuntimeConfig::default().to_value().unwrap(),
            json!({ "busIdleThreshold": "-120", "workerWarningThreshold": "85" })
        );
    }

    #[test]
    fn dsp_runtime_config_rejects_unknown_or_invalid_values() {
        assert!(DspRuntimeConfig::from_value(&json!({
            "workerWarningThreshold": "86"
        }))
        .is_err());
        assert!(DspRuntimeConfig::from_value(&json!({ "unknown": "value" })).is_err());
    }

    #[test]
    fn bus_idle_threshold_maps_to_linear_amplitude() {
        assert_eq!(BusIdleThreshold::Exact.amplitude(), 0.0);
        assert_eq!(BusIdleThreshold::Db140.amplitude(), 0.0000001);
        assert_eq!(BusIdleThreshold::Db120.amplitude(), 0.000001);
        assert_eq!(BusIdleThreshold::Db100.amplitude(), 0.00001);
        assert_eq!(BusIdleThreshold::Db80.amplitude(), 0.0001);
    }
}
