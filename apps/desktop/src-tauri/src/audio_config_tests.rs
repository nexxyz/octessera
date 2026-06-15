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

#[test]
fn synth_payload_includes_master_fx_slots() {
    let config = AudioInstrumentsConfig {
        instruments: vec![AudioInstrumentSlotConfig {
            kind: "synth".to_string(),
            synth: None,
            sample: None,
            mixer: None,
        }],
        mixer: Some(AudioMixerConfig {
            buses: vec![AudioBusConfig {
                slot1: Some(serde_json::json!({
                    "type": "delay",
                    "params": { "timeMs": 333.0, "feedback": 0.42, "mixPct": 44.0 }
                })),
                slot2: None,
                pan_pos: Some(18),
            }],
            master: Some(AudioMasterConfig {
                slots: vec![serde_json::json!({
                    "type": "eq",
                    "params": {
                        "lowGainDb": 3.0,
                        "midGainDb": 0.0,
                        "midFreqHz": 1200.0,
                        "midQ": 1.0,
                        "highGainDb": -2.0,
                        "mixPct": 100.0
                    }
                })],
            }),
        }),
        pan_positions: None,
        master_volume: None,
    };

    let payload = synth_payload(&config);
    let mixer = payload.mixer.expect("expected mixer config");
    assert_eq!(mixer.buses.len(), 1);
    assert_eq!(mixer.buses[0].pan_pos, 18);
    match &mixer.buses[0].slots[0] {
        FxBusSlotConfig::Config { kind, params } => {
            assert_eq!(kind, "delay");
            assert_eq!(params["feedback"], serde_json::json!(0.42));
        }
        slot => panic!("unexpected bus slot: {slot:?}"),
    }
    let master = mixer.master.expect("expected master FX config");
    assert_eq!(master.slots.len(), 1);
    match &master.slots[0] {
        FxBusSlotConfig::Config { kind, params } => {
            assert_eq!(kind, "eq");
            assert_eq!(params["lowGainDb"], serde_json::json!(3.0));
        }
        slot => panic!("unexpected slot: {slot:?}"),
    }
}
