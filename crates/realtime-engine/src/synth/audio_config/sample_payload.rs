//! A Sampler instrument's `sample` block: slot paths, each slot's start and loop, and how the
//! bank shapes what it plays.

use super::super::types::{
    EnvConfig, FilterType, SampleBankConfig, SampleLoopMode, SamplePlaybackConfig,
    SampleShapeConfig, SampleSlotConfig, SAMPLE_SLOTS_PER_INSTRUMENT,
};
use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct NormalizedSampleConfig {
    pub slots: Vec<Option<String>>,
    /// Each slot's start and loop; the loop is already off unless the instrument holds notes.
    pub slot_playback: [SamplePlaybackConfig; SAMPLE_SLOTS_PER_INSTRUMENT],
    pub tune_semis: f32,
    pub gain_pct: f32,
    pub velocity_sensitivity_pct: f32,
    pub filter_cutoff_hz: f32,
    pub filter_resonance: f32,
    pub shape: SampleShapeConfig,
}

impl NormalizedSampleConfig {
    /// The bank a host builds once it has loaded (or failed to load) each slot's buffer.
    pub fn bank(&self, mut slots: Vec<SampleSlotConfig>) -> SampleBankConfig {
        for (slot, playback) in slots.iter_mut().zip(self.slot_playback) {
            slot.playback = playback;
        }
        SampleBankConfig {
            slots,
            tune_semis: self.tune_semis,
            gain_pct: self.gain_pct,
            velocity_sensitivity_pct: self.velocity_sensitivity_pct,
            filter_cutoff_hz: self.filter_cutoff_hz,
            filter_resonance: self.filter_resonance,
            shape: self.shape,
        }
    }
}

#[derive(Clone, Deserialize)]
pub(super) struct AudioSamplePayload {
    #[serde(default)]
    slots: Vec<AudioSampleSlotPayload>,
    #[serde(default, rename = "tuneSemis")]
    tune_semis: Option<f32>,
    #[serde(default)]
    amp: Option<AudioSampleAmpPayload>,
    #[serde(default)]
    filter: Option<AudioSampleFilterPayload>,
    #[serde(default, rename = "ampEnv")]
    amp_env: Option<EnvConfig>,
    #[serde(default, rename = "filterEnv")]
    filter_env: Option<EnvConfig>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioSampleSlotPayload {
    #[serde(default)]
    path: Option<String>,
    #[serde(default, rename = "loop")]
    loop_mode: Option<String>,
    #[serde(default)]
    start_pct: Option<f32>,
    #[serde(default)]
    loop_start_pct: Option<f32>,
    #[serde(default)]
    loop_end_pct: Option<f32>,
}

#[derive(Clone, Deserialize)]
struct AudioSampleAmpPayload {
    #[serde(default, rename = "gainPct")]
    gain_pct: Option<f32>,
    #[serde(default, rename = "velocitySensitivityPct")]
    velocity_sensitivity_pct: Option<f32>,
}

#[derive(Clone, Deserialize)]
struct AudioSampleFilterPayload {
    #[serde(default, rename = "type")]
    kind: Option<FilterType>,
    #[serde(default, rename = "cutoffHz")]
    cutoff_hz: Option<f32>,
    #[serde(default)]
    resonance: Option<f32>,
    #[serde(default, rename = "envAmountPct")]
    env_amount_pct: Option<f32>,
    #[serde(default, rename = "keyTrackingPct")]
    key_tracking_pct: Option<f32>,
}

/// `holds_notes` is Note Mode `hold`: only a held note can loop, so a oneshot slot plays once.
pub(super) fn normalize_sample(
    sample: AudioSamplePayload,
    holds_notes: bool,
) -> Result<NormalizedSampleConfig, String> {
    let shape = normalize_sample_shape(&sample);
    let mut slot_playback = [SamplePlaybackConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT];
    for (playback, slot) in slot_playback.iter_mut().zip(&sample.slots) {
        *playback = normalize_slot_playback(slot, holds_notes)?;
    }
    Ok(NormalizedSampleConfig {
        slots: sample
            .slots
            .into_iter()
            .take(SAMPLE_SLOTS_PER_INSTRUMENT)
            .map(|slot| slot.path)
            .collect(),
        slot_playback,
        tune_semis: sample.tune_semis.unwrap_or(0.0),
        gain_pct: sample
            .amp
            .as_ref()
            .and_then(|amp| amp.gain_pct)
            .unwrap_or(100.0),
        velocity_sensitivity_pct: sample
            .amp
            .as_ref()
            .and_then(|amp| amp.velocity_sensitivity_pct)
            .unwrap_or(100.0),
        filter_cutoff_hz: sample
            .filter
            .as_ref()
            .and_then(|filter| filter.cutoff_hz)
            .unwrap_or(20_000.0),
        filter_resonance: sample
            .filter
            .as_ref()
            .and_then(|filter| filter.resonance)
            .unwrap_or(0.0),
        shape,
    })
}

fn normalize_slot_playback(
    slot: &AudioSampleSlotPayload,
    holds_notes: bool,
) -> Result<SamplePlaybackConfig, String> {
    let neutral = SamplePlaybackConfig::default();
    let loop_mode = match slot.loop_mode.as_deref() {
        None | Some("off") => SampleLoopMode::Off,
        Some("forward") => SampleLoopMode::Forward,
        Some("pingpong") => SampleLoopMode::PingPong,
        Some(other) => return Err(format!("unsupported sample loop `{other}`")),
    };
    let pct = |value: Option<f32>, fallback: f32| value.unwrap_or(fallback).clamp(0.0, 100.0);
    let loop_start_pct = pct(slot.loop_start_pct, neutral.loop_start_pct);
    let loop_end_pct = pct(slot.loop_end_pct, neutral.loop_end_pct);
    if loop_end_pct <= loop_start_pct {
        return Err("sample Loop End must be above Loop Start".into());
    }
    Ok(SamplePlaybackConfig {
        loop_mode: if holds_notes {
            loop_mode
        } else {
            SampleLoopMode::Off
        },
        start_pct: pct(slot.start_pct, neutral.start_pct),
        loop_start_pct,
        loop_end_pct,
    })
}

fn normalize_sample_shape(sample: &AudioSamplePayload) -> SampleShapeConfig {
    let neutral = SampleShapeConfig::default();
    let filter = sample.filter.as_ref();
    SampleShapeConfig {
        filter_type: filter
            .and_then(|filter| filter.kind)
            .unwrap_or(neutral.filter_type),
        filter_env_amount_pct: filter
            .and_then(|filter| filter.env_amount_pct)
            .map_or(neutral.filter_env_amount_pct, |pct| {
                pct.clamp(-100.0, 100.0)
            }),
        filter_key_tracking_pct: filter
            .and_then(|filter| filter.key_tracking_pct)
            .map_or(neutral.filter_key_tracking_pct, |pct| pct.clamp(0.0, 100.0)),
        amp_env: sample.amp_env.unwrap_or(neutral.amp_env),
        filter_env: sample.filter_env.unwrap_or(neutral.filter_env),
    }
}
