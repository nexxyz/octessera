use serde::{Deserialize, Serialize};
use std::f32::consts::PI;
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

pub const INSTRUMENT_SLOT_COUNT: usize = 8;
pub const VOICES_PER_SLOT: usize = 8;
pub const BUS_SLOTS_PER_BUS: usize = 2;
pub const DEFAULT_PAN_POSITIONS: usize = 33;
pub const SAMPLE_SLOTS_PER_INSTRUMENT: usize = 8;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MomentaryFxTarget {
    Global,
    FxBus { index: usize },
    Instrument { index: usize },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VoiceStealingMode {
    Off,
    Lenient,
    Balanced,
    Aggressive,
}

#[derive(Clone, Copy, Debug)]
pub struct AudioLoadStatus {
    pub ratio: f32,
    pub voice_steal: bool,
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
}

impl Default for SampleBankConfig {
    fn default() -> Self {
        Self {
            slots: vec![SampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT],
            tune_semis: 0.0,
            gain_pct: 100.0,
            velocity_sensitivity_pct: 100.0,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EnvStage {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct EnvState {
    pub(super) stage: EnvStage,
    pub(super) level: f32,
    pub(super) stage_pos: u32,
    pub(super) stage_len: u32,
    pub(super) sustain: f32,
    pub(super) release_start: f32,
}

impl EnvState {
    pub(super) fn note_on(cfg: EnvConfig, sample_rate: u32) -> Self {
        let a = ms_to_samples(cfg.attack_ms, sample_rate);
        let d = ms_to_samples(cfg.decay_ms, sample_rate);
        let sustain = (cfg.sustain_pct / 100.0).clamp(0.0, 1.0);
        let stage = if a == 0 {
            EnvStage::Decay
        } else {
            EnvStage::Attack
        };
        let stage_len = if stage == EnvStage::Attack { a } else { d };
        Self {
            stage,
            level: if stage == EnvStage::Attack { 0.0 } else { 1.0 },
            stage_pos: 0,
            stage_len,
            sustain,
            release_start: 0.0,
        }
    }

    pub(super) fn begin_release(&mut self, cfg: EnvConfig, sample_rate: u32) {
        if self.stage == EnvStage::Release || self.stage == EnvStage::Off {
            return;
        }
        self.stage = EnvStage::Release;
        self.stage_pos = 0;
        self.stage_len = ms_to_samples(cfg.release_ms, sample_rate).max(1);
        self.release_start = self.level;
    }

    pub(super) fn next(&mut self) -> f32 {
        match self.stage {
            EnvStage::Attack => {
                if self.stage_len == 0 {
                    self.stage = EnvStage::Decay;
                    self.stage_pos = 0;
                    self.stage_len = 0;
                    self.level = 1.0;
                    return self.level;
                }
                let t = (self.stage_pos as f32) / (self.stage_len as f32);
                self.level = t.clamp(0.0, 1.0);
                self.stage_pos = self.stage_pos.saturating_add(1);
                if self.stage_pos >= self.stage_len {
                    self.stage = EnvStage::Decay;
                    self.stage_pos = 0;
                    self.stage_len = 0;
                    self.level = 1.0;
                }
                self.level
            }
            EnvStage::Decay => {
                if self.stage_len == 0 {
                    self.stage = EnvStage::Sustain;
                    self.level = self.sustain;
                    return self.level;
                }
                let t = (self.stage_pos as f32) / (self.stage_len as f32);
                self.level = (1.0 + (self.sustain - 1.0) * t).clamp(0.0, 1.0);
                self.stage_pos = self.stage_pos.saturating_add(1);
                if self.stage_pos >= self.stage_len {
                    self.stage = EnvStage::Sustain;
                    self.level = self.sustain;
                }
                self.level
            }
            EnvStage::Sustain => self.level,
            EnvStage::Release => {
                let t = (self.stage_pos as f32) / (self.stage_len as f32);
                self.level = (self.release_start * (1.0 - t)).clamp(0.0, 1.0);
                self.stage_pos = self.stage_pos.saturating_add(1);
                if self.stage_pos >= self.stage_len {
                    self.stage = EnvStage::Off;
                    self.level = 0.0;
                }
                self.level
            }
            EnvStage::Off => 0.0,
        }
    }

    pub(super) fn is_off(&self) -> bool {
        self.stage == EnvStage::Off
    }

    pub(super) fn is_releasing(&self) -> bool {
        self.stage == EnvStage::Release
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BiquadState {
    pub(super) x1: f32,
    pub(super) x2: f32,
    pub(super) y1: f32,
    pub(super) y2: f32,
}

impl BiquadState {
    pub(super) fn new() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub(super) fn process(
        &mut self,
        x: f32,
        mode: FilterType,
        cutoff_hz: f32,
        q: f32,
        sample_rate: u32,
    ) -> f32 {
        let cutoff = cutoff_hz.clamp(20.0, 20_000.0);
        let qv = q.clamp(0.25, 20.0);
        let w0 = 2.0 * PI * cutoff / (sample_rate as f32);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * qv);

        let (b0, b1, b2, a0, a1, a2) = match mode {
            FilterType::Lowpass => (
                (1.0 - cos_w0) * 0.5,
                1.0 - cos_w0,
                (1.0 - cos_w0) * 0.5,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterType::Highpass => (
                (1.0 + cos_w0) * 0.5,
                -(1.0 + cos_w0),
                (1.0 + cos_w0) * 0.5,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterType::Bandpass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha),
            FilterType::Notch => (
                1.0,
                -2.0 * cos_w0,
                1.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
        };

        let nb0 = b0 / a0;
        let nb1 = b1 / a0;
        let nb2 = b2 / a0;
        let na1 = a1 / a0;
        let na2 = a2 / a0;
        let y = nb0 * x + nb1 * self.x1 + nb2 * self.x2 - na1 * self.y1 - na2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Voice {
    pub(super) active: bool,
    pub(super) instrument_slot: u8,
    pub(super) midi_note: u8,
    pub(super) velocity: u8,
    pub(super) note_off_sample: u64,
    pub(super) started_sample: u64,
    pub(super) freq_hz: f32,
    pub(super) phase1: f32,
    pub(super) phase2: f32,
    pub(super) amp_env: EnvState,
    pub(super) filt_env: EnvState,
    pub(super) filt: BiquadState,
}

impl Voice {
    pub(super) fn off() -> Self {
        Self {
            active: false,
            instrument_slot: 0,
            midi_note: 0,
            velocity: 0,
            note_off_sample: 0,
            started_sample: 0,
            freq_hz: 440.0,
            phase1: 0.0,
            phase2: 0.0,
            amp_env: EnvState {
                stage: EnvStage::Off,
                level: 0.0,
                stage_pos: 0,
                stage_len: 0,
                sustain: 0.0,
                release_start: 0.0,
            },
            filt_env: EnvState {
                stage: EnvStage::Off,
                level: 0.0,
                stage_pos: 0,
                stage_len: 0,
                sustain: 0.0,
                release_start: 0.0,
            },
            filt: BiquadState::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct InstrumentMod {
    pub(super) cutoff_cc: f32,
    pub(super) resonance_cc: f32,
}

impl InstrumentMod {
    pub(super) fn new() -> Self {
        Self {
            cutoff_cc: 0.0,
            resonance_cc: 0.0,
        }
    }
}

pub(super) fn ms_to_samples(ms: f32, sample_rate: u32) -> u32 {
    if ms <= 0.0 {
        return 0;
    }
    ((ms / 1000.0) * (sample_rate as f32)).round().max(0.0) as u32
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
