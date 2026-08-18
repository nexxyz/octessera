use super::mixer::InstrumentMixerDto;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) note_behavior: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) auto_name: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) synth: Option<SynthDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sample: Option<SampleDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) midi: Option<MidiInstrumentDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) midi_engine: Option<MidiInstrumentDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) mixer: Option<InstrumentMixerDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) selected_slot: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) base_velocity: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) slots: Option<Vec<SampleSlotDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) assignments: Option<Vec<SampleAssignmentDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tune_semis: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) amp: Option<SampleAmpDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) amp_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) filter: Option<FilterDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) filter_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity_levels_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity_levels: Option<VelocityLevelsDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SampleSlotDto {
    pub(super) path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleAssignmentDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) x: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) y: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sample_slot: Option<usize>,
    pub(super) level: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleAmpDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) gain_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity_sensitivity_pct: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VelocityLevelsDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) high: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) medium: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) low: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) osc1: Option<SynthOscDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) osc2: Option<SynthOscDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) amp: Option<SynthAmpDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) amp_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) filter: Option<FilterDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) filter_env: Option<EnvelopeDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthOscDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) waveform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) level_pct: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) octave: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) detune_cents: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) pulse_width_pct: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthAmpDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) gain_pct: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity_sensitivity_pct: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) attack_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) decay_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sustain_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) release_ms: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cutoff_hz: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) resonance: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) env_amount_pct: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) key_tracking_pct: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiInstrumentDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) channel: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) duration_ms: Option<u16>,
}
