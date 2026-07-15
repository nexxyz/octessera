use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WaveformId {
    Sine,
    Triangle,
    Saw,
    Square,
    Pulse,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilterType {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
}

pub const VOICES_PER_SLOT: usize = 8;
pub const BUS_SLOTS_PER_BUS: usize = 2;
include!(concat!(
    env!("OUT_DIR"),
    "/synth_platform_capabilities.generated.rs"
));

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MomentaryFxTarget {
    Global,
    FxBus { index: usize },
    Instrument { index: usize },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VoiceStealingMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "fixed12")]
    Fixed12,
    #[serde(rename = "fixed16")]
    Fixed16,
    #[serde(rename = "auto-soft")]
    AutoSoft,
    #[serde(rename = "auto-balanced")]
    AutoBalanced,
    #[serde(rename = "auto-hard")]
    AutoHard,
}

#[derive(Clone, Copy, Debug)]
pub struct AudioLoadStatus {
    pub ratio: f32,
    pub voice_steal: bool,
    pub block_ratio_p95: f32,
    pub block_ratio_max: f32,
    pub blocks: u64,
    pub control_events: u64,
    pub config_events: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SynthProfileSnapshot {
    pub active_synth_voices: usize,
    pub active_sample_voices: usize,
    pub active_preview_sample_voices: usize,
    pub active_momentary_fx: usize,
    pub cumulative_voice_steals: u64,
    pub synth_parallel_dispatches: u64,
    pub synth_parallel_light_skips: u64,
    pub synth_parallel_backoff_skips: u64,
    pub synth_parallel_timing_backoffs: u64,
    pub synth_parallel_failures: u64,
    pub synth_parallel_unhealthy: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EnvConfig {
    #[serde(rename = "attackMs")]
    pub attack_ms: f32,
    #[serde(rename = "decayMs")]
    pub decay_ms: f32,
    #[serde(rename = "sustainPct")]
    pub sustain_pct: f32,
    #[serde(rename = "releaseMs")]
    pub release_ms: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OscConfig {
    pub waveform: WaveformId,
    #[serde(rename = "levelPct")]
    pub level_pct: f32,
    pub octave: i32,
    #[serde(rename = "detuneCents")]
    pub detune_cents: f32,
    #[serde(rename = "pulseWidthPct")]
    pub pulse_width_pct: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FilterConfig {
    #[serde(rename = "type")]
    pub kind: FilterType,
    #[serde(rename = "cutoffHz")]
    pub cutoff_hz: f32,
    pub resonance: f32,
    #[serde(rename = "envAmountPct")]
    pub env_amount_pct: f32,
    #[serde(rename = "keyTrackingPct")]
    pub key_tracking_pct: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SynthConfig {
    pub osc1: OscConfig,
    pub osc2: OscConfig,
    pub amp: AmpConfig,
    #[serde(rename = "ampEnv")]
    pub amp_env: EnvConfig,
    pub filter: FilterConfig,
    #[serde(rename = "filterEnv")]
    pub filter_env: EnvConfig,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AmpConfig {
    #[serde(rename = "gainPct")]
    pub gain_pct: f32,
    #[serde(rename = "velocitySensitivityPct")]
    pub velocity_sensitivity_pct: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstrumentSlotConfig {
    #[serde(rename = "type")]
    pub kind: String,
    pub synth: SynthConfig,
    #[serde(default)]
    pub mixer: Option<InstrumentMixerConfig>,
}

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
}

pub const RENDER_PROFILE_STAGE_COUNT: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderProfileSnapshot {
    pub enabled: bool,
    pub frames_observed: u64,
    pub blocks_observed: u64,
    pub last_block_frames: usize,
    pub last_frame_total_ns: u64,
    pub last_block_total_ns: u64,
    pub stage_ns: [u64; RENDER_PROFILE_STAGE_COUNT],
    pub interleave_ns: u64,
}

impl Default for SampleBankConfig {
    fn default() -> Self {
        Self {
            slots: vec![SampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT],
            tune_semis: 0.0,
            gain_pct: 100.0,
            velocity_sensitivity_pct: 100.0,
            filter_cutoff_hz: 8000.0,
            filter_resonance: 20.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstrumentMixerConfig {
    pub route: String,
    #[serde(rename = "panPos")]
    pub pan_pos: usize,
    #[serde(default = "default_mixer_volume")]
    pub volume: f32,
}

fn default_mixer_volume() -> f32 {
    100.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FxBusSlotConfig {
    Kind(String),
    Config {
        #[serde(rename = "type", default = "default_fx_bus_slot_type")]
        kind: String,
        #[serde(default)]
        params: std::collections::BTreeMap<String, serde_json::Value>,
    },
}

fn default_fx_bus_slot_type() -> String {
    "none".to_string()
}

impl FxBusSlotConfig {
    pub(super) fn kind_str(&self) -> &str {
        match self {
            FxBusSlotConfig::Kind(s) => s.as_str(),
            FxBusSlotConfig::Config { kind, .. } => kind.as_str(),
        }
    }

    pub(super) fn params(&self) -> Option<&std::collections::BTreeMap<String, serde_json::Value>> {
        match self {
            FxBusSlotConfig::Kind(_) => None,
            FxBusSlotConfig::Config { params, .. } => Some(params),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FxBusConfig {
    #[serde(default)]
    pub slots: Vec<FxBusSlotConfig>,
    #[serde(rename = "panPos")]
    pub pan_pos: usize,
    #[serde(default = "default_fx_bus_volume", rename = "volumePct")]
    pub volume_pct: f32,
}

fn default_fx_bus_volume() -> f32 {
    100.0
}

impl Default for FxBusConfig {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            pan_pos: 16,
            volume_pct: default_fx_bus_volume(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MasterFxConfig {
    #[serde(default)]
    pub slots: Vec<FxBusSlotConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MixerConfig {
    #[serde(default)]
    pub buses: Vec<FxBusConfig>,
    #[serde(default)]
    pub master: Option<MasterFxConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstrumentsConfig {
    pub instruments: Vec<InstrumentSlotConfig>,
    #[serde(default)]
    pub mixer: Option<MixerConfig>,
    #[serde(default = "default_pan_positions", rename = "panPositions")]
    pub pan_positions: usize,
    #[serde(default = "default_master_volume", rename = "masterVolume")]
    pub master_volume: f32,
}

fn default_pan_positions() -> usize {
    DEFAULT_PAN_POSITIONS
}

fn default_master_volume() -> f32 {
    100.0
}

pub fn default_synth_config() -> SynthConfig {
    SynthConfig {
        osc1: OscConfig {
            waveform: WaveformId::Saw,
            level_pct: 80.0,
            octave: 0,
            detune_cents: 0.0,
            pulse_width_pct: 50.0,
        },
        osc2: OscConfig {
            waveform: WaveformId::Square,
            level_pct: 80.0,
            octave: 0,
            detune_cents: 0.0,
            pulse_width_pct: 50.0,
        },
        amp: AmpConfig {
            gain_pct: 80.0,
            velocity_sensitivity_pct: 100.0,
        },
        amp_env: EnvConfig {
            attack_ms: 5.0,
            decay_ms: 120.0,
            sustain_pct: 70.0,
            release_ms: 180.0,
        },
        filter: FilterConfig {
            kind: FilterType::Lowpass,
            cutoff_hz: 8000.0,
            resonance: 20.0,
            env_amount_pct: 0.0,
            key_tracking_pct: 0.0,
        },
        filter_env: EnvConfig {
            attack_ms: 5.0,
            decay_ms: 120.0,
            sustain_pct: 70.0,
            release_ms: 180.0,
        },
    }
}
