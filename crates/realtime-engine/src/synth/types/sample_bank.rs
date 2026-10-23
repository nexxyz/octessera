//! A sampler slot's loaded buffers and how they are played: tune, gain, filter and envelopes.

use super::{EnvConfig, FilterType, SAMPLE_SLOTS_PER_INSTRUMENT};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct SampleBuffer {
    pub samples: Arc<[f32]>,
    pub channels: u16,
    pub sample_rate: u32,
}

#[derive(Clone, Debug, Default)]
pub struct SampleSlotConfig {
    pub buffer: Option<SampleBuffer>,
}

#[derive(Clone, Debug)]
pub struct SampleBankConfig {
    pub slots: Vec<SampleSlotConfig>,
    pub tune_semis: f32,
    pub gain_pct: f32,
    pub velocity_sensitivity_pct: f32,
    pub filter_cutoff_hz: f32,
    pub filter_resonance: f32,
    pub shape: SampleShapeConfig,
}

/// How a sample is shaped after it is read: filter type, the filter's envelope and key
/// tracking, and the amp envelope. The neutral default plays a sample exactly as recorded.
#[derive(Clone, Copy, Debug)]
pub struct SampleShapeConfig {
    pub filter_type: FilterType,
    pub filter_env_amount_pct: f32,
    pub filter_key_tracking_pct: f32,
    pub amp_env: EnvConfig,
    pub filter_env: EnvConfig,
}

impl Default for SampleShapeConfig {
    fn default() -> Self {
        Self {
            filter_type: FilterType::Lowpass,
            filter_env_amount_pct: 0.0,
            filter_key_tracking_pct: 0.0,
            amp_env: EnvConfig {
                attack_ms: 0.0,
                decay_ms: 0.0,
                sustain_pct: 100.0,
                release_ms: 5.0,
            },
            filter_env: EnvConfig {
                attack_ms: 5.0,
                decay_ms: 120.0,
                sustain_pct: 70.0,
                release_ms: 180.0,
            },
        }
    }
}

impl Default for SampleBankConfig {
    fn default() -> Self {
        Self {
            slots: vec![SampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT],
            tune_semis: 0.0,
            gain_pct: 100.0,
            velocity_sensitivity_pct: 100.0,
            filter_cutoff_hz: 20_000.0,
            filter_resonance: 0.0,
            shape: SampleShapeConfig::default(),
        }
    }
}
