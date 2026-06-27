use crate::audio::AudioService;
use realtime_engine::synth::{
    default_synth_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig, MixerConfig, SampleBankConfig,
    SampleBuffer, SampleSlotConfig, SynthConfig, VoiceStealingMode, DEFAULT_PAN_POSITIONS,
    SAMPLE_SLOTS_PER_INSTRUMENT,
};
use rodio::Source;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::path::{Component, Path, PathBuf};

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

#[derive(Deserialize)]
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

#[derive(Deserialize)]
pub(crate) struct AudioSamplePayload {
    #[serde(default)]
    slots: Vec<AudioSampleSlotPayload>,
    #[serde(default, rename = "tuneSemis")]
    tune_semis: Option<f32>,
    #[serde(default)]
    amp: Option<AudioSampleAmpPayload>,
}

#[derive(Deserialize)]
struct AudioSampleSlotPayload {
    #[serde(default)]
    path: Option<String>,
}

#[derive(Deserialize)]
struct AudioSampleAmpPayload {
    #[serde(default, rename = "gainPct")]
    gain_pct: Option<f32>,
    #[serde(default, rename = "velocitySensitivityPct")]
    velocity_sensitivity_pct: Option<f32>,
}

#[derive(Deserialize)]
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
    slot1: Option<serde_json::Value>,
    #[serde(default)]
    slot2: Option<serde_json::Value>,
    #[serde(default)]
    pan_pos: Option<usize>,
}

#[derive(Deserialize)]
struct AudioMasterPayload {
    #[serde(default)]
    slots: Vec<serde_json::Value>,
}

pub(crate) struct ParsedAudioConfig {
    pub(crate) instruments: InstrumentsConfig,
    pub(crate) sample_sources: Vec<SampleSource>,
    pub(crate) voice_stealing_mode: Option<String>,
}

pub(crate) struct SampleSource {
    kind: String,
    sample: Option<AudioSamplePayload>,
}

pub(crate) fn parse_audio_config(config: &serde_json::Value) -> Result<ParsedAudioConfig, String> {
    let config = serde_json::from_value::<AudioConfigPayload>(config.clone())
        .map_err(|e| format!("invalid audio config payload: {e}"))?;
    let mut sample_sources = Vec::with_capacity(config.instruments.len());
    let instruments = InstrumentsConfig {
        instruments: config
            .instruments
            .into_iter()
            .map(|slot| {
                let AudioInstrumentPayload {
                    kind,
                    synth,
                    mixer,
                    sample,
                } = slot;
                sample_sources.push(SampleSource {
                    kind: kind.clone(),
                    sample,
                });
                InstrumentSlotConfig {
                    kind,
                    synth: synth.unwrap_or_else(default_synth_config),
                    mixer: Some(InstrumentMixerConfig {
                        route: mixer
                            .as_ref()
                            .and_then(|m| m.route.clone())
                            .unwrap_or_else(|| "direct".to_string()),
                        pan_pos: mixer.as_ref().and_then(|m| m.pan_pos).unwrap_or(16),
                        volume: mixer.as_ref().and_then(|m| m.volume).unwrap_or(100.0),
                    }),
                }
            })
            .collect(),
        mixer: config.mixer.map(mixer_config),
        pan_positions: config.pan_positions.unwrap_or(DEFAULT_PAN_POSITIONS),
        master_volume: config.master_volume.unwrap_or(100.0),
    };
    Ok(ParsedAudioConfig {
        instruments,
        sample_sources,
        voice_stealing_mode: config.voice_stealing_mode,
    })
}

pub(crate) fn parse_voice_stealing_mode(raw: &str) -> VoiceStealingMode {
    match raw {
        "none" | "off" => VoiceStealingMode::None,
        "fixed12" => VoiceStealingMode::Fixed12,
        "fixed16" => VoiceStealingMode::Fixed16,
        "auto-soft" | "lenient" => VoiceStealingMode::AutoSoft,
        "auto-hard" | "aggressive" => VoiceStealingMode::AutoHard,
        _ => VoiceStealingMode::AutoBalanced,
    }
}

pub(crate) fn sample_banks(
    samples: &[SampleSource],
    samples_dir: &Path,
    audio: &AudioService,
) -> Vec<SampleBankConfig> {
    samples
        .iter()
        .map(|source| {
            let Some(sample) = source.active_sampler_payload() else {
                return SampleBankConfig::default();
            };
            let mut slots = vec![SampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT];
            for (index, entry) in sample
                .slots
                .iter()
                .enumerate()
                .take(SAMPLE_SLOTS_PER_INSTRUMENT)
            {
                let Some(path) = entry
                    .path
                    .as_deref()
                    .and_then(|path| resolve_sample_path(samples_dir, path))
                else {
                    continue;
                };
                slots[index].buffer = cached_sample_buffer(audio, &path);
            }
            SampleBankConfig {
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
            }
        })
        .collect()
}

fn cached_sample_buffer(audio: &AudioService, path: &Path) -> Option<SampleBuffer> {
    let key = path.to_string_lossy().to_string();
    if let Ok(cache) = audio.sample_cache.lock() {
        if let Some(buffer) = cache.get(&key) {
            return Some(buffer.clone());
        }
    } else {
        return None;
    }
    let buffer = decode_sample_file(path)?;
    if let Ok(mut cache) = audio.sample_cache.lock() {
        cache.insert(key, buffer.clone());
    }
    Some(buffer)
}

pub(crate) fn sample_signature(samples: &[SampleSource]) -> String {
    samples
        .iter()
        .map(|source| {
            let Some(sample) = source.active_sampler_payload() else {
                return "-".to_string();
            };
            let amp = sample.amp.as_ref();
            let paths = sample
                .slots
                .iter()
                .map(|slot| slot.path.as_deref().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("|");
            format!(
                "{paths}|t={}|g={}|v={}",
                sample.tune_semis.unwrap_or(0.0),
                amp.and_then(|amp| amp.gain_pct).unwrap_or(100.0),
                amp.and_then(|amp| amp.velocity_sensitivity_pct)
                    .unwrap_or(100.0)
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

impl SampleSource {
    fn active_sampler_payload(&self) -> Option<&AudioSamplePayload> {
        (self.kind == "sampler")
            .then_some(self.sample.as_ref())
            .flatten()
    }
}

fn resolve_sample_path(samples_dir: &Path, path: &str) -> Option<PathBuf> {
    let relative = Path::new(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return None;
    }
    Some(samples_dir.join(relative))
}

fn decode_sample_file(path: &Path) -> Option<SampleBuffer> {
    let file = File::open(path).ok()?;
    let decoder = rodio::Decoder::new(BufReader::new(file)).ok()?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples = decoder.convert_samples::<f32>().collect::<Vec<_>>();
    if samples.is_empty() {
        return None;
    }
    Some(SampleBuffer {
        samples: samples.into(),
        channels,
        sample_rate,
    })
}

fn mixer_config(config: AudioMixerPayload) -> MixerConfig {
    MixerConfig {
        buses: config
            .buses
            .into_iter()
            .map(|bus| FxBusConfig {
                slots: [bus.slot1, bus.slot2]
                    .into_iter()
                    .flatten()
                    .map(fx_slot_config)
                    .collect(),
                pan_pos: bus.pan_pos.unwrap_or(16),
            })
            .collect(),
        master: config.master.map(|master| MasterFxConfig {
            slots: master.slots.into_iter().map(fx_slot_config).collect(),
        }),
    }
}

fn fx_slot_config(value: serde_json::Value) -> FxBusSlotConfig {
    serde_json::from_value(value).unwrap_or_else(|_| FxBusSlotConfig::Kind("none".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_audio_config_payload_for_engine() {
        let config = parse_audio_config(&serde_json::json!({
            "masterVolume": 81,
            "voiceStealingMode": "fixed12",
            "panPositions": 33,
            "instruments": [{
                "type": "synth",
                "mixer": { "route": "fx_bus_1", "panPos": 12, "volume": 76 }
            }],
            "mixer": {
                "buses": [{ "slot1": { "type": "none" }, "slot2": { "type": "none" }, "panPos": 16 }],
                "master": { "slots": [{ "type": "none" }] }
            }
        }))
        .unwrap();

        assert_eq!(config.instruments.master_volume, 81.0);
        assert_eq!(config.instruments.pan_positions, 33);
        assert_eq!(config.instruments.instruments[0].kind, "synth");
        let mixer = config.instruments.instruments[0].mixer.as_ref().unwrap();
        assert_eq!(mixer.route, "fx_bus_1");
        assert_eq!(mixer.pan_pos, 12);
        assert_eq!(mixer.volume, 76.0);
        assert_eq!(config.instruments.mixer.unwrap().buses.len(), 1);
        assert_eq!(config.voice_stealing_mode.as_deref(), Some("fixed12"));
        assert_eq!(
            parse_voice_stealing_mode(config.voice_stealing_mode.as_deref().unwrap()),
            VoiceStealingMode::Fixed12
        );
    }

    #[test]
    fn sample_signature_tracks_sampler_param_changes() {
        let first = vec![sampler_source(AudioSamplePayload {
            slots: vec![AudioSampleSlotPayload {
                path: Some("kick.wav".into()),
            }],
            tune_semis: Some(0.0),
            amp: Some(AudioSampleAmpPayload {
                gain_pct: Some(100.0),
                velocity_sensitivity_pct: Some(100.0),
            }),
        })];
        let changed = vec![sampler_source(AudioSamplePayload {
            slots: vec![AudioSampleSlotPayload {
                path: Some("kick.wav".into()),
            }],
            tune_semis: Some(2.0),
            amp: Some(AudioSampleAmpPayload {
                gain_pct: Some(80.0),
                velocity_sensitivity_pct: Some(70.0),
            }),
        })];

        assert_ne!(sample_signature(&first), sample_signature(&changed));
    }

    #[test]
    fn sample_signature_ignores_sample_payload_for_non_sampler_slots() {
        let synth_with_sample_payload = vec![SampleSource {
            kind: "synth".into(),
            sample: Some(AudioSamplePayload {
                slots: vec![AudioSampleSlotPayload {
                    path: Some("kick.wav".into()),
                }],
                tune_semis: Some(12.0),
                amp: Some(AudioSampleAmpPayload {
                    gain_pct: Some(50.0),
                    velocity_sensitivity_pct: Some(60.0),
                }),
            }),
        }];

        assert_eq!(sample_signature(&synth_with_sample_payload), "-");
    }

    fn sampler_source(sample: AudioSamplePayload) -> SampleSource {
        SampleSource {
            kind: "sampler".into(),
            sample: Some(sample),
        }
    }
}
