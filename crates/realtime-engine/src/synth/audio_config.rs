use super::types::{
    default_synth_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig, MixerConfig, SynthConfig,
    VoiceStealingMode, DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT, SAMPLE_SLOTS_PER_INSTRUMENT,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct NormalizedAudioConfig {
    pub instruments: Vec<NormalizedInstrumentSlot>,
    pub mixer: Option<MixerConfig>,
    pub pan_positions: usize,
    pub master_volume: f32,
    pub voice_stealing_mode: Option<VoiceStealingMode>,
}

#[derive(Clone, Debug)]
pub struct NormalizedInstrumentSlot {
    pub kind: String,
    pub slot: InstrumentSlotConfig,
    pub sample: Option<NormalizedSampleConfig>,
}

#[derive(Clone, Debug)]
pub struct NormalizedSampleConfig {
    pub slots: Vec<Option<String>>,
    pub tune_semis: f32,
    pub gain_pct: f32,
    pub velocity_sensitivity_pct: f32,
    pub filter_cutoff_hz: f32,
    pub filter_resonance: f32,
}

impl NormalizedAudioConfig {
    pub fn instruments_config(&self) -> InstrumentsConfig {
        InstrumentsConfig {
            instruments: self
                .instruments
                .iter()
                .map(|instrument| instrument.slot.clone())
                .collect(),
            mixer: self.mixer.clone(),
            pan_positions: self.pan_positions,
            master_volume: self.master_volume,
        }
    }

    pub fn sample_bank_signature(&self) -> String {
        self.instruments
            .iter()
            .take(INSTRUMENT_SLOT_COUNT)
            .map(|instrument| {
                let Some(sample) = instrument.active_sample() else {
                    return "-".to_string();
                };
                let paths = sample
                    .slots
                    .iter()
                    .map(|path| path.as_deref().unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join("|");
                format!(
                    "{paths}|t={}|g={}|v={}|fc={}|fr={}",
                    sample.tune_semis,
                    sample.gain_pct,
                    sample.velocity_sensitivity_pct,
                    sample.filter_cutoff_hz,
                    sample.filter_resonance
                )
            })
            .collect::<Vec<_>>()
            .join(";")
    }
}

impl NormalizedInstrumentSlot {
    pub fn active_sample(&self) -> Option<&NormalizedSampleConfig> {
        (self.kind == "sampler")
            .then_some(self.sample.as_ref())
            .flatten()
    }
}

#[derive(Deserialize)]
struct AudioConfigPayload {
    instruments: Vec<AudioInstrumentPayload>,
    #[serde(default)]
    mixer: Option<AudioMixerPayload>,
    #[serde(default, rename = "panPositions")]
    pan_positions: Option<usize>,
    #[serde(default, rename = "masterVolume")]
    master_volume: Option<f32>,
    #[serde(default, rename = "voiceStealingMode")]
    voice_stealing_mode: Option<String>,
}

#[derive(Clone, Deserialize)]
struct AudioInstrumentPayload {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    synth: Option<SynthConfig>,
    #[serde(default)]
    mixer: Option<AudioInstrumentMixerPayload>,
    #[serde(default)]
    sample: Option<AudioSamplePayload>,
}

#[derive(Clone, Deserialize)]
struct AudioSamplePayload {
    #[serde(default)]
    slots: Vec<AudioSampleSlotPayload>,
    #[serde(default, rename = "tuneSemis")]
    tune_semis: Option<f32>,
    #[serde(default)]
    amp: Option<AudioSampleAmpPayload>,
    #[serde(default)]
    filter: Option<AudioSampleFilterPayload>,
}

#[derive(Clone, Deserialize)]
struct AudioSampleSlotPayload {
    #[serde(default)]
    path: Option<String>,
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
    #[serde(default, rename = "cutoffHz")]
    cutoff_hz: Option<f32>,
    #[serde(default)]
    resonance: Option<f32>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioInstrumentMixerPayload {
    #[serde(default)]
    route: Option<String>,
    #[serde(default)]
    pan_pos: Option<usize>,
    #[serde(default)]
    volume: Option<f32>,
}

#[derive(Deserialize)]
struct AudioMixerPayload {
    #[serde(default)]
    buses: Vec<AudioBusPayload>,
    #[serde(default)]
    master: Option<AudioMasterPayload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioBusPayload {
    #[serde(default)]
    slot1: Option<Value>,
    #[serde(default)]
    slot2: Option<Value>,
    #[serde(default)]
    slot3: Option<Value>,
    #[serde(default)]
    pan_pos: Option<usize>,
    #[serde(default)]
    volume_pct: Option<f32>,
}

#[derive(Deserialize)]
struct AudioMasterPayload {
    #[serde(default)]
    slots: Vec<Value>,
}

pub fn normalize_audio_config(config: &Value) -> Result<NormalizedAudioConfig, String> {
    let config = serde_json::from_value::<AudioConfigPayload>(config.clone())
        .map_err(|error| format!("invalid audio config payload: {error}"))?;
    let mixer = config.mixer.map(normalize_mixer).transpose()?;
    let instruments = config
        .instruments
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            normalize_instrument_slot(slot)
                .map_err(|error| format!("invalid instrument {}: {error}", index + 1))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NormalizedAudioConfig {
        instruments,
        mixer,
        pan_positions: config.pan_positions.unwrap_or(DEFAULT_PAN_POSITIONS),
        master_volume: config.master_volume.unwrap_or(100.0),
        voice_stealing_mode: config
            .voice_stealing_mode
            .as_deref()
            .map(parse_voice_stealing_mode),
    })
}

pub fn normalize_instrument_slot_config(
    config: &Value,
) -> Result<NormalizedInstrumentSlot, String> {
    let slot = serde_json::from_value::<AudioInstrumentPayload>(config.clone())
        .map_err(|error| format!("invalid instrument slot payload: {error}"))?;
    normalize_instrument_slot(slot)
}

pub fn normalize_fx_slot(value: &Value) -> Result<FxBusSlotConfig, String> {
    match value {
        Value::String(kind) => {
            validate_fx_type(kind)?;
            Ok(FxBusSlotConfig::Kind(kind.clone()))
        }
        Value::Object(object) => {
            let kind = object
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| "expected slot object with string type".to_string())?;
            validate_fx_type(kind)?;
            let params = object
                .get("params")
                .cloned()
                .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
            let params = serde_json::from_value::<BTreeMap<String, Value>>(params)
                .map_err(|error| format!("invalid slot params: {error}"))?;
            Ok(FxBusSlotConfig::Config {
                kind: kind.to_string(),
                params,
            })
        }
        _ => Err("expected slot string or object".to_string()),
    }
}

pub fn validate_fx_type(kind: &str) -> Result<(), String> {
    if matches!(
        kind,
        "none"
            | "tremolo"
            | "delay"
            | "vibrato"
            | "chorus"
            | "flanger"
            | "filter_lfo"
            | "wah"
            | "reverb"
            | "glitch"
            | "auto_pan"
            | "duck"
            | "saturator"
            | "distortion"
            | "bitcrusher"
            | "compressor"
            | "eq"
            | "vinyl"
    ) {
        return Ok(());
    }
    Err(format!("unsupported FX type `{kind}`"))
}

pub fn validate_momentary_fx_type(kind: &str) -> Result<(), String> {
    if matches!(kind, "stutter" | "freeze" | "filter_sweep" | "pitch_shift") {
        return Ok(());
    }
    Err(format!("unsupported momentary FX type `{kind}`"))
}

pub fn validate_synth_param_path(path: &str) -> Result<(), String> {
    if matches!(
        path,
        "synth.amp.gainPct"
            | "synth.amp.velocitySensitivityPct"
            | "synth.ampEnv.attackMs"
            | "synth.ampEnv.decayMs"
            | "synth.ampEnv.sustainPct"
            | "synth.ampEnv.releaseMs"
            | "synth.filter.cutoffHz"
            | "synth.filter.resonance"
            | "synth.filter.envAmountPct"
            | "synth.filter.keyTrackingPct"
            | "synth.filterEnv.attackMs"
            | "synth.filterEnv.decayMs"
            | "synth.filterEnv.sustainPct"
            | "synth.filterEnv.releaseMs"
    ) {
        return Ok(());
    }
    Err(format!("unsupported synth parameter path `{path}`"))
}

pub fn validate_sample_bank_param_path(path: &str) -> Result<(), String> {
    if matches!(
        path,
        "sample.tuneSemis"
            | "sample.amp.gainPct"
            | "sample.amp.velocitySensitivityPct"
            | "sample.filter.cutoffHz"
            | "sample.filter.resonance"
    ) {
        return Ok(());
    }
    Err(format!("unsupported sample parameter path `{path}`"))
}

pub fn parse_voice_stealing_mode(raw: &str) -> VoiceStealingMode {
    match raw {
        "none" | "off" => VoiceStealingMode::None,
        "fixed12" => VoiceStealingMode::Fixed12,
        "fixed16" => VoiceStealingMode::Fixed16,
        "auto-soft" | "lenient" => VoiceStealingMode::AutoSoft,
        "auto-hard" | "aggressive" => VoiceStealingMode::AutoHard,
        _ => VoiceStealingMode::AutoBalanced,
    }
}

fn normalize_instrument_slot(
    slot: AudioInstrumentPayload,
) -> Result<NormalizedInstrumentSlot, String> {
    let AudioInstrumentPayload {
        kind,
        synth,
        mixer,
        sample,
    } = slot;
    let normalized_sample = sample.map(normalize_sample);
    Ok(NormalizedInstrumentSlot {
        slot: InstrumentSlotConfig {
            kind: kind.clone(),
            synth: synth.unwrap_or_else(default_synth_config),
            mixer: Some(InstrumentMixerConfig {
                route: mixer
                    .as_ref()
                    .and_then(|mixer| mixer.route.clone())
                    .unwrap_or_else(|| "direct".to_string()),
                pan_pos: mixer.as_ref().and_then(|mixer| mixer.pan_pos).unwrap_or(16),
                volume: mixer
                    .as_ref()
                    .and_then(|mixer| mixer.volume)
                    .unwrap_or(100.0),
            }),
        },
        kind,
        sample: normalized_sample,
    })
}

fn normalize_sample(sample: AudioSamplePayload) -> NormalizedSampleConfig {
    NormalizedSampleConfig {
        slots: sample
            .slots
            .into_iter()
            .take(SAMPLE_SLOTS_PER_INSTRUMENT)
            .map(|slot| slot.path)
            .collect(),
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
            .unwrap_or(8000.0),
        filter_resonance: sample
            .filter
            .as_ref()
            .and_then(|filter| filter.resonance)
            .unwrap_or(20.0),
    }
}

fn normalize_mixer(config: AudioMixerPayload) -> Result<MixerConfig, String> {
    let buses = config
        .buses
        .into_iter()
        .enumerate()
        .map(|(bus_index, bus)| {
            let slots = [bus.slot1, bus.slot2, bus.slot3]
                .into_iter()
                .enumerate()
                .map(|(slot_index, slot)| {
                    slot.map(|value| {
                        normalize_fx_slot(&value).map_err(|error| {
                            format!(
                                "invalid mixer bus {} slot {}: {error}",
                                bus_index + 1,
                                slot_index + 1
                            )
                        })
                    })
                    .transpose()
                    .map(|slot| slot.unwrap_or_else(|| FxBusSlotConfig::Kind("none".into())))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(FxBusConfig {
                slots,
                pan_pos: bus.pan_pos.unwrap_or(16),
                volume_pct: bus.volume_pct.unwrap_or(100.0),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let master = config
        .master
        .map(|master| {
            master
                .slots
                .into_iter()
                .take(2)
                .enumerate()
                .map(|(slot_index, slot)| {
                    normalize_fx_slot(&slot)
                        .map_err(|error| format!("invalid master slot {}: {error}", slot_index + 1))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|slots| MasterFxConfig { slots })
        })
        .transpose()?;
    Ok(MixerConfig { buses, master })
}

#[cfg(test)]
#[path = "audio_config_tests.rs"]
mod tests;
