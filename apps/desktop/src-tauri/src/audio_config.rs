use crate::SampleSlotConfig;
use realtime_engine::synth::{
    FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MixerConfig, SampleBankConfig, SampleBuffer, SampleSlotConfig as EngineSampleSlotConfig,
    SynthConfig, VoiceStealingMode, INSTRUMENT_SLOT_COUNT, SAMPLE_SLOTS_PER_INSTRUMENT,
};
use rodio::Source;
use serde::Deserialize;
use std::fmt::Write;
use std::fs::File;
use std::io::BufReader;

#[derive(Deserialize)]
pub struct AudioInstrumentsConfig {
    pub instruments: Vec<AudioInstrumentSlotConfig>,
    #[serde(default)]
    mixer: Option<AudioMixerConfig>,
    #[serde(default, rename = "panPositions")]
    pan_positions: Option<usize>,
    #[serde(default, rename = "masterVolume")]
    master_volume: Option<f32>,
}

#[derive(Deserialize)]
struct AudioMixerConfig {
    #[serde(default)]
    buses: Vec<AudioBusConfig>,
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
#[serde(rename_all = "camelCase")]
pub struct AudioRuntimePolicyConfig {
    pub voice_stealing_mode: String,
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
            .map(|slot| InstrumentSlotConfig {
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
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: mixer_buses(config),
        }),
        pan_positions: config.pan_positions.unwrap_or(realtime_engine::synth::DEFAULT_PAN_POSITIONS),
        master_volume: config.master_volume.unwrap_or(100.0),
    }
}

pub fn parse_voice_stealing_mode(raw: &str) -> VoiceStealingMode {
    match raw {
        "off" => VoiceStealingMode::Off,
        "lenient" => VoiceStealingMode::Lenient,
        "aggressive" => VoiceStealingMode::Aggressive,
        _ => VoiceStealingMode::Balanced,
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
             if slot.kind != "sampler" {
                 return SampleBankConfig::default();
             }
            let Some(sample) = &slot.sample else {
                return SampleBankConfig::default();
            };
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

fn bus_slot(value: &Option<serde_json::Value>) -> FxBusSlotConfig {
    value
        .clone()
        .and_then(|v| serde_json::from_value::<FxBusSlotConfig>(v).ok())
        .unwrap_or_else(|| FxBusSlotConfig::Kind("none".to_string()))
}

pub fn decode_sample_file(path: &str) -> Option<SampleBuffer> {
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
mod tests {
    use super::*;

    #[test]
    fn build_audio_slot_configs_applies_defaults_and_limits() {
        let mut many: Vec<AudioInstrumentSlotConfig> = Vec::new();
        many.push(AudioInstrumentSlotConfig {
            kind: "sampler".to_string(),
            synth: None,
            sample: Some(AudioSampleConfig {
                slots: vec![
                    AudioSampleSlotEntry {
                        path: Some("a.wav".to_string()),
                    },
                    AudioSampleSlotEntry {
                        path: Some("b.wav".to_string()),
                    },
                ],
                tune_semis: Some(7.0),
                amp: Some(AudioAmpConfig {
                    gain_pct: Some(80.0),
                    velocity_sensitivity_pct: Some(50.0),
                }),
            }),
            mixer: None,
        });
        many.push(AudioInstrumentSlotConfig {
            kind: "synth".to_string(),
            synth: None,
            sample: None,
            mixer: None,
        });
        for _ in 0..20 {
            many.push(AudioInstrumentSlotConfig {
                kind: "sampler".to_string(),
                synth: None,
                sample: None,
                mixer: None,
            });
        }

        let (slots, cfgs) = build_audio_slot_configs(&many);
        assert!(!slots[0]);
        assert!(slots[1]);
        assert_eq!(cfgs[0].tune_semis, 7.0);
        assert_eq!(cfgs[0].gain_pct, 80.0);
        assert_eq!(cfgs[0].vel_sens_pct, 50.0);
        assert_eq!(cfgs[0].slots[0], Some("a.wav".to_string()));
        assert_eq!(cfgs[0].slots[1], Some("b.wav".to_string()));
        assert_eq!(slots.len(), INSTRUMENT_SLOT_COUNT);
    }

   #[test]
     fn sample_banks_preserve_sample_playback_controls_without_decoding_in_audio_thread() {
        let config = AudioInstrumentsConfig {
            instruments: vec![AudioInstrumentSlotConfig {
                kind: "sampler".to_string(),
                synth: None,
                sample: Some(AudioSampleConfig {
                    slots: vec![AudioSampleSlotEntry {
                        path: Some("missing.wav".to_string()),
                    }],
                    tune_semis: Some(-5.0),
                    amp: Some(AudioAmpConfig {
                        gain_pct: Some(70.0),
                        velocity_sensitivity_pct: Some(40.0),
                    }),
                }),
                mixer: None,
            }],
            mixer: None,
            pan_positions: None,
            master_volume: None,
        };

        let banks = sample_banks(&config, |_| None, |_| None);

        assert_eq!(banks.len(), 1);
        assert_eq!(banks[0].tune_semis, -5.0);
        assert_eq!(banks[0].gain_pct, 70.0);
        assert_eq!(banks[0].velocity_sensitivity_pct, 40.0);
        assert!(banks[0].slots[0].buffer.is_none());
    }

    #[test]
    fn sample_bank_signature_ignores_synth_only_changes() {
        let mut synth = realtime_engine::synth::default_synth_config();
        let config = AudioInstrumentsConfig {
            instruments: vec![
                AudioInstrumentSlotConfig {
                    kind: "synth".to_string(),
                    synth: Some(synth),
                    sample: None,
                    mixer: None,
                },
                AudioInstrumentSlotConfig {
                    kind: "sampler".to_string(),
                    synth: None,
                    sample: Some(AudioSampleConfig {
                        slots: vec![AudioSampleSlotEntry {
                            path: Some("kick.wav".to_string()),
                        }],
                        tune_semis: Some(0.0),
                        amp: Some(AudioAmpConfig {
                            gain_pct: Some(100.0),
                            velocity_sensitivity_pct: Some(100.0),
                        }),
                    }),
                    mixer: None,
                },
            ],
            mixer: None,
            pan_positions: None,
            master_volume: None,
        };
        let before = sample_bank_signature(&config);
        synth.filter.cutoff_hz = 120.0;
        let changed_synth = AudioInstrumentsConfig {
            instruments: vec![
               AudioInstrumentSlotConfig {
                    kind: "synth".to_string(),
                    synth: Some(synth),
                    sample: None,
                    mixer: None,
                },
                AudioInstrumentSlotConfig {
                    kind: "sampler".to_string(),
                    synth: None,
                    sample: Some(AudioSampleConfig {
                        slots: vec![AudioSampleSlotEntry {
                            path: Some("kick.wav".to_string()),
                        }],
                        tune_semis: Some(0.0),
                        amp: Some(AudioAmpConfig {
                            gain_pct: Some(100.0),
                            velocity_sensitivity_pct: Some(100.0),
                        }),
                    }),
                    mixer: None,
                },
            ],
             mixer: None,
             pan_positions: None,
             master_volume: None,
        };
        assert_eq!(before, sample_bank_signature(&changed_synth));
    }

    #[test]
    fn sample_bank_signature_ignores_synth_only_changes_2() {
        let config = AudioInstrumentsConfig {
            instruments: vec![
                AudioInstrumentSlotConfig {
                    kind: "synth".to_string(),
                    synth: Some(realtime_engine::synth::default_synth_config()),
                    sample: None,
                    mixer: None,
                },
                AudioInstrumentSlotConfig {
                    kind: "sampler".to_string(),
                    synth: None,
                    sample: Some(AudioSampleConfig {
                        slots: vec![AudioSampleSlotEntry {
                            path: Some("kick.wav".to_string()),
                        }],
                        tune_semis: Some(0.0),
                        amp: Some(AudioAmpConfig {
                            gain_pct: Some(100.0),
                            velocity_sensitivity_pct: Some(100.0),
                        }),
                    }),
                    mixer: None,
                },
            ],
            mixer: None,
            pan_positions: None,
            master_volume: None,
        };
        let before = sample_bank_signature(&config);
        let synth = realtime_engine::synth::default_synth_config();
        let changed_synth = AudioInstrumentsConfig {
            instruments: vec![
                AudioInstrumentSlotConfig {
                    kind: "synth".to_string(),
                    synth: Some(synth),
                    sample: None,
                    mixer: None,
                },
                AudioInstrumentSlotConfig {
                    kind: "sampler".to_string(),
                    synth: None,
                    sample: Some(AudioSampleConfig {
                        slots: vec![AudioSampleSlotEntry {
                            path: Some("kick.wav".to_string()),
                        }],
                        tune_semis: Some(0.0),
                        amp: Some(AudioAmpConfig {
                            gain_pct: Some(100.0),
                            velocity_sensitivity_pct: Some(100.0),
                        }),
                    }),
                    mixer: None,
                },
            ],
            mixer: None,
            pan_positions: None,
            master_volume: None,
        };

        assert_eq!(before, sample_bank_signature(&changed_synth));
    }
}
