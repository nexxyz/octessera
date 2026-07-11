use crate::SampleSlotConfig;
use realtime_engine::synth::{
    FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, SampleBankConfig, SampleBuffer,
    SampleSlotConfig as EngineSampleSlotConfig, SynthConfig, INSTRUMENT_SLOT_COUNT,
    SAMPLE_SLOTS_PER_INSTRUMENT,
};
use serde::Deserialize;
use std::fmt::Write;

mod sample_decode;
mod voice_stealing;

pub use sample_decode::decode_sample_file;
pub use voice_stealing::parse_voice_stealing_mode;

#[derive(Deserialize)]
pub struct AudioInstrumentsConfig {
    pub instruments: Vec<AudioInstrumentSlotConfig>,
    #[serde(default)]
    mixer: Option<AudioMixerConfig>,
    #[serde(default, rename = "panPositions")]
    pan_positions: Option<usize>,
    #[serde(default, rename = "masterVolume")]
    master_volume: Option<f32>,
    #[serde(default, rename = "voiceStealingMode")]
    pub voice_stealing_mode: Option<String>,
}

#[derive(Deserialize)]
struct AudioMixerConfig {
    #[serde(default)]
    buses: Vec<AudioBusConfig>,
    #[serde(default)]
    master: Option<AudioMasterConfig>,
}

#[derive(Deserialize)]
struct AudioMasterConfig {
    #[serde(default)]
    slots: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioBusConfig {
    #[serde(default)]
    slot1: Option<serde_json::Value>,
    #[serde(default)]
    slot2: Option<serde_json::Value>,
    #[serde(default)]
    pan_pos: Option<usize>,
}

#[derive(Deserialize)]
pub struct AudioInstrumentSlotConfig {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    synth: Option<SynthConfig>,
    #[serde(default)]
    sample: Option<AudioSampleConfig>,
    #[serde(default)]
    mixer: Option<AudioInstrumentMixerConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioInstrumentMixerConfig {
    #[serde(default)]
    route: Option<String>,
    #[serde(default)]
    pan_pos: Option<usize>,
    #[serde(default)]
    volume: Option<f32>,
}

#[derive(Deserialize)]
struct AudioSampleConfig {
    #[serde(default)]
    slots: Vec<AudioSampleSlotEntry>,
    #[serde(default, rename = "tuneSemis")]
    tune_semis: Option<f32>,
    #[serde(default)]
    amp: Option<AudioAmpConfig>,
}

#[derive(Deserialize)]
struct AudioSampleSlotEntry {
    #[serde(default)]
    path: Option<String>,
}

#[derive(Deserialize)]
struct AudioAmpConfig {
    #[serde(default, rename = "gainPct")]
    gain_pct: Option<f32>,
    #[serde(default, rename = "velocitySensitivityPct")]
    velocity_sensitivity_pct: Option<f32>,
}

pub fn synth_payload(config: &AudioInstrumentsConfig) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: config
            .instruments
            .iter()
            .map(instrument_slot_config)
            .collect(),
        mixer: Some(MixerConfig {
            buses: mixer_buses(config),
            master: Some(MasterFxConfig {
                slots: master_slots(config),
            }),
        }),
        pan_positions: config
            .pan_positions
            .unwrap_or(realtime_engine::synth::DEFAULT_PAN_POSITIONS),
        master_volume: config.master_volume.unwrap_or(100.0),
    }
}

pub fn parse_instrument_slot_config(
    config: &serde_json::Value,
) -> Result<InstrumentSlotConfig, String> {
    let slot = serde_json::from_value::<AudioInstrumentSlotConfig>(config.clone())
        .map_err(|e| format!("invalid instrument slot payload: {e}"))?;
    Ok(instrument_slot_config(&slot))
}

pub(crate) fn sample_bank_for_slot_config(
    config: &serde_json::Value,
    resolve_sample: impl Fn(&str) -> Option<String>,
    load_sample: impl FnMut(&str) -> Option<SampleBuffer>,
) -> Result<Option<SampleBankConfig>, String> {
    let slot = serde_json::from_value::<AudioInstrumentSlotConfig>(config.clone())
        .map_err(|e| format!("invalid instrument slot payload: {e}"))?;
    Ok(sample_bank_for_slot(&slot, resolve_sample, load_sample))
}

fn instrument_slot_config(slot: &AudioInstrumentSlotConfig) -> InstrumentSlotConfig {
    InstrumentSlotConfig {
        kind: slot.kind.clone(),
        synth: slot
            .synth
            .unwrap_or_else(realtime_engine::synth::default_synth_config),
        mixer: Some(InstrumentMixerConfig {
            route: slot
                .mixer
                .as_ref()
                .and_then(|m| m.route.clone())
                .unwrap_or_else(|| "direct".to_string()),
            pan_pos: slot.mixer.as_ref().and_then(|m| m.pan_pos).unwrap_or(16),
            volume: slot.mixer.as_ref().and_then(|m| m.volume).unwrap_or(100.0),
        }),
    }
}

pub fn build_audio_slot_configs(
    instruments: &[AudioInstrumentSlotConfig],
) -> (
    [bool; INSTRUMENT_SLOT_COUNT],
    [SampleSlotConfig; INSTRUMENT_SLOT_COUNT],
) {
    let mut synth_slots = [false; INSTRUMENT_SLOT_COUNT];
    let mut sample_cfgs = std::array::from_fn(|_| SampleSlotConfig::default());
    for (idx, slot) in instruments.iter().enumerate() {
        if idx >= INSTRUMENT_SLOT_COUNT {
            break;
        }
        synth_slots[idx] = slot.kind == "synth";
        if slot.kind != "sampler" {
            continue;
        }
        sample_cfgs[idx] = sample_slot_config(slot);
    }
    (synth_slots, sample_cfgs)
}

pub fn sample_banks(
    config: &AudioInstrumentsConfig,
    resolve_sample: impl Fn(&str) -> Option<String>,
    mut load_sample: impl FnMut(&str) -> Option<SampleBuffer>,
) -> Vec<SampleBankConfig> {
    config
        .instruments
        .iter()
        .take(INSTRUMENT_SLOT_COUNT)
        .map(|slot| {
            sample_bank_for_slot(slot, &resolve_sample, &mut load_sample).unwrap_or_default()
        })
        .collect()
}

fn sample_bank_for_slot(
    slot: &AudioInstrumentSlotConfig,
    resolve_sample: impl Fn(&str) -> Option<String>,
    mut load_sample: impl FnMut(&str) -> Option<SampleBuffer>,
) -> Option<SampleBankConfig> {
    if slot.kind != "sampler" {
        return None;
    }
    let sample = slot.sample.as_ref()?;
    let mut slots = vec![EngineSampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT];
    for (idx, entry) in sample
        .slots
        .iter()
        .enumerate()
        .take(SAMPLE_SLOTS_PER_INSTRUMENT)
    {
        let Some(path) = &entry.path else {
            continue;
        };
        let Some(full_path) = resolve_sample(path) else {
            continue;
        };
        slots[idx].buffer = load_sample(&full_path);
    }
    Some(SampleBankConfig {
        slots,
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
    })
}

pub fn sample_bank_signature(config: &AudioInstrumentsConfig) -> String {
    let mut out = String::new();
    for slot in config.instruments.iter().take(INSTRUMENT_SLOT_COUNT) {
        if slot.kind != "sampler" {
            out.push_str("-|;");
            continue;
        }
        let Some(sample) = &slot.sample else {
            out.push_str("sample:none;");
            continue;
        };
        let _ = write!(
            out,
            "sample:t{}:g{}:v{}:",
            sample.tune_semis.unwrap_or(0.0),
            sample
                .amp
                .as_ref()
                .and_then(|amp| amp.gain_pct)
                .unwrap_or(100.0),
            sample
                .amp
                .as_ref()
                .and_then(|amp| amp.velocity_sensitivity_pct)
                .unwrap_or(100.0)
        );
        for entry in sample.slots.iter().take(SAMPLE_SLOTS_PER_INSTRUMENT) {
            out.push_str(entry.path.as_deref().unwrap_or(""));
            out.push('|');
        }
        out.push(';');
    }
    out
}

fn mixer_buses(config: &AudioInstrumentsConfig) -> Vec<FxBusConfig> {
    config
        .mixer
        .as_ref()
        .map(|m| {
            m.buses
                .iter()
                .map(|b| FxBusConfig {
                    slots: vec![bus_slot(&b.slot1), bus_slot(&b.slot2)],
                    pan_pos: b.pan_pos.unwrap_or(16),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn master_slots(config: &AudioInstrumentsConfig) -> Vec<FxBusSlotConfig> {
    config
        .mixer
        .as_ref()
        .and_then(|m| m.master.as_ref())
        .map(|master| {
            master
                .slots
                .iter()
                .map(|slot| bus_slot(&Some(slot.clone())))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn bus_slot(value: &Option<serde_json::Value>) -> FxBusSlotConfig {
    value
        .clone()
        .and_then(|v| serde_json::from_value::<FxBusSlotConfig>(v).ok())
        .unwrap_or_else(|| FxBusSlotConfig::Kind("none".to_string()))
}

fn sample_slot_config(slot: &AudioInstrumentSlotConfig) -> SampleSlotConfig {
    let mut out = SampleSlotConfig::default();
    if let Some(s) = &slot.sample {
        out.tune_semis = s.tune_semis.unwrap_or(0.0);
        if let Some(amp) = &s.amp {
            out.gain_pct = amp.gain_pct.unwrap_or(100.0);
            out.vel_sens_pct = amp.velocity_sensitivity_pct.unwrap_or(100.0);
        }
        for (i, entry) in s.slots.iter().enumerate().take(8) {
            out.slots[i] = entry.path.clone();
        }
    }
    out
}

#[cfg(test)]
#[path = "audio_config_tests.rs"]
mod tests;
