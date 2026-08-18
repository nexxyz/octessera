use super::ParamModsDto;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) worlds: Option<WorldsDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) pulses: Option<PulsesDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) param_mods: Option<ParamModsDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) auto_name: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldsDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) behavior_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "deserialize_nullable_value")]
    pub(super) behavior_config: Option<Option<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "deserialize_nullable_value")]
    pub(super) saved_state: Option<Option<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "deserialize_nullable_value")]
    pub(super) behavior_state: Option<Option<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) behavior_config_history: Option<std::collections::BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) save_grid_state: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) step_rate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) trigger_gates: Option<Vec<bool>>,
}

fn deserialize_nullable_value<'de, D>(deserializer: D) -> Result<Option<Option<Value>>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Some(Option::<Value>::deserialize(deserializer)?))
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PulsesDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scan_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scan_axis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scan_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scan_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scan_sections: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) event_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) trigger_probability_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) trigger_probability_low_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) trigger_probability_high_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) state_notes_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) trigger_probability_map: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) mapping: Option<MappingDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) pitch: Option<PitchDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) x: Option<AxisDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) y: Option<AxisDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) arp: Option<ArpDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MappingDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scanned: Option<MappingEventDto>,
    #[serde(rename = "scanned_empty")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scanned_empty: Option<MappingEventDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) activate: Option<MappingEventDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) stable: Option<MappingEventDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) deactivate: Option<MappingEventDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingEventDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) slot: Option<SlotDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) delay_steps: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) retrigger_count: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SlotDto {
    Index(usize),
    None(String),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PitchDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) lowest_note: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) highest_note: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) starting_note: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) out_of_range: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AxisDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) from: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) to: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) pitch: Option<PitchAxisDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity: Option<ValueLaneDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) filter_cutoff: Option<ValueLaneDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) filter_resonance: Option<ValueLaneDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PitchAxisDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) steps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) restart_each_section: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueLaneDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) from: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) to: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) grid_offset: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) curve: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArpDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) step_interval_steps: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) note_length_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) gate_pct: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) octave_spread: Option<i64>,
}
