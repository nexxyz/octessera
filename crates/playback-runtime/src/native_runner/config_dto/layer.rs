use super::ParamModsDto;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) build: Option<BuildDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) link: Option<LinkDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) param_mods: Option<ParamModsDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) auto_name: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) behavior_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "super::deserialize_nullable")]
    pub(in crate::native_runner) behavior_config: Option<Option<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "super::deserialize_nullable")]
    pub(in crate::native_runner) saved_state: Option<Option<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) behavior_config_history:
        Option<std::collections::BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) seeded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) step_rate: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) scan_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) scan_axis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) scan_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) scan_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) scan_sections: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) event_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) seeded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) trigger_probability_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) trigger_probability_low_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) trigger_probability_high_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) trigger_probability_note_offs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) state_notes_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) trigger_probability_map: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) mapping: Option<MappingDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) pitch: Option<PitchDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) x: Option<AxisDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) y: Option<AxisDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) arp: Option<ArpDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MappingDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) scanned: Option<MappingEventDto>,
    #[serde(rename = "scanned_empty")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) scanned_empty: Option<MappingEventDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) activate: Option<MappingEventDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) stable: Option<MappingEventDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) deactivate: Option<MappingEventDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingEventDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) slot: Option<SlotDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) delay_steps: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) retrigger_count: Option<u8>,
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
    pub(in crate::native_runner) lowest_note: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) highest_note: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) starting_note: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) scale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) out_of_range: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AxisDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) from: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) to: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) pitch: Option<PitchAxisDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity: Option<ValueLaneDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter_cutoff: Option<ValueLaneDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter_resonance: Option<ValueLaneDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PitchAxisDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) steps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) restart_each_section: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueLaneDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) from: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) to: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) grid_offset: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) curve: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArpDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) step_interval_steps: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) note_length_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) gate_pct: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) octave_spread: Option<i64>,
}
