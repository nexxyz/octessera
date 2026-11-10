use super::mixer::InstrumentMixerDto;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) note_behavior: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) auto_name: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) synth: Option<SynthDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) fm: Option<FmDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) pluck: Option<PluckDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) drum: Option<DrumDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tile_stamp: Option<TileStampDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) sample: Option<SampleDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) midi: Option<MidiInstrumentDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) midi_drum: Option<MidiDrumDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) mixer: Option<InstrumentMixerDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileStampDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tune_semis: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tune_cents: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) level: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) selected_slot: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) base_velocity: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) slots: Option<Vec<SampleSlotDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) assignments: Option<Vec<SampleAssignmentDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tune_semis: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) amp: Option<SampleAmpDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) amp_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter: Option<FilterDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity_levels_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity_levels: Option<VelocityLevelsDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) follow_transpose: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) follow_pitch: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleSlotDto {
    pub(in crate::native_runner) path: Option<String>,
    #[serde(rename = "loop", skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) loop_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) start_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) loop_start_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) loop_end_pct: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleAssignmentDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) x: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) y: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) sample_slot: Option<usize>,
    pub(in crate::native_runner) level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tune_semis: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tune_cents: Option<i8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleAmpDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) gain_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity_sensitivity_pct: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VelocityLevelsDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) high: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) medium: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) low: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) osc1: Option<SynthOscDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) osc2: Option<SynthOscDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) amp: Option<SynthAmpDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) amp_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter: Option<FilterDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter_env: Option<EnvelopeDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FmDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) ratio_fine_cents: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) index: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity_to_index_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) mod_shape_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) mod_mix_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) index_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) amp: Option<SynthAmpDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) amp_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter: Option<FilterDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter_env: Option<EnvelopeDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluckDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) decay_ms: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) brightness_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) pick_position_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) pick_depth_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) dispersion_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) body_amount_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) body_frequency_hz: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) amp: Option<SynthAmpDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) amp_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter: Option<FilterDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter_env: Option<EnvelopeDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrumDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) voices: Option<Vec<DrumVoiceDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) assignments: Option<Vec<DrumAssignmentDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) amp: Option<SynthAmpDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) amp_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter: Option<FilterDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) filter_env: Option<EnvelopeDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity_levels: Option<VelocityLevelsDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) follow_transpose: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) follow_pitch: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrumVoiceDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) sound: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tune_semis: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) decay_ms: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tone_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) attack_ms: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) sweep_semis: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) sweep_ms: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) noise_mix_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) level_pct: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrumAssignmentDto {
    pub(in crate::native_runner) x: Option<u8>,
    pub(in crate::native_runner) y: Option<u8>,
    pub(in crate::native_runner) voice: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tune_semis: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tune_cents: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) level: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthOscDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) waveform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) level_pct: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) octave: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) detune_cents: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) pulse_width_pct: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthAmpDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) gain_pct: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity_sensitivity_pct: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) attack_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) decay_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) sustain_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) release_ms: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) cutoff_hz: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) resonance: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) env_amount_pct: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) key_tracking_pct: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiInstrumentDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) channel: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) duration_ms: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiDrumDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) channel: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) duration_ms: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tile_note: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) tile_velocity: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) assignments: Option<Vec<MidiDrumAssignmentDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MidiDrumAssignmentDto {
    pub(in crate::native_runner) x: Option<u8>,
    pub(in crate::native_runner) y: Option<u8>,
    pub(in crate::native_runner) note: Option<u8>,
    pub(in crate::native_runner) velocity: Option<u8>,
}
