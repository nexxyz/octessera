use super::super::*;

#[test]
fn master_volume_controls_final_output_gain() {
    let mut full = SynthEngine::new(48_000);
    full.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "sampler".to_string(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "direct".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut half = SynthEngine::new(48_000);
    half.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "sampler".to_string(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "direct".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 50.0,
    });
    full.set_sample_banks(vec![sample_bank(vec![1.0, 1.0, 1.0, 1.0])]);
    half.set_sample_banks(vec![sample_bank(vec![1.0, 1.0, 1.0, 1.0])]);
    full.note_on(0, 36, 127, 1_000);
    half.note_on(0, 36, 127, 1_000);

    let mut full_sum = 0.0_f32;
    let mut half_sum = 0.0_f32;
    for _ in 0..4 {
        full_sum += full.next_sample().abs();
        half_sum += half.next_sample().abs();
    }

    assert!(
        half_sum > full_sum * 0.45 && half_sum < full_sum * 0.55,
        "master volume should scale final output gain"
    );
}

#[test]
fn master_fx_processes_bus_routed_output() {
    let mut dry = SynthEngine::new(48_000);
    dry.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".to_string(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Kind("none".to_string())],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut wet = SynthEngine::new(48_000);
    wet.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".to_string(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Kind("none".to_string())],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
            }],
            master: Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "saturator".to_string(),
                    params: BTreeMap::from([
                        ("drive".to_string(), json!(5.0)),
                        ("mixPct".to_string(), json!(100.0)),
                    ]),
                }],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });

    dry.note_on(0, 60, 127, 500);
    wet.note_on(0, 60, 127, 500);
    let mut dry_sum = 0.0_f32;
    let mut wet_sum = 0.0_f32;
    for _ in 0..4096 {
        dry_sum += dry.next_sample().abs();
        wet_sum += wet.next_sample().abs();
    }

    assert!((wet_sum - dry_sum).abs() > dry_sum * 0.03);
}

#[test]
fn master_compressor_is_stereo_linked() {
    let mut right_only = SynthEngine::new(48_000);
    right_only.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: 0,
                    volume: 0.0,
                }),
            },
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: DEFAULT_PAN_POSITIONS - 1,
                    volume: 25.0,
                }),
            },
        ],
        mixer: Some(MixerConfig {
            buses: vec![],
            master: Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "compressor".to_string(),
                    params: BTreeMap::from([
                        ("thresholdDb".to_string(), json!(-40.0)),
                        ("ratio".to_string(), json!(12.0)),
                        ("attackMs".to_string(), json!(1.0)),
                        ("releaseMs".to_string(), json!(40.0)),
                        ("makeupDb".to_string(), json!(0.0)),
                        ("mixPct".to_string(), json!(100.0)),
                    ]),
                }],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut linked = SynthEngine::new(48_000);
    linked.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: 0,
                    volume: 100.0,
                }),
            },
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: DEFAULT_PAN_POSITIONS - 1,
                    volume: 25.0,
                }),
            },
        ],
        mixer: Some(MixerConfig {
            buses: vec![],
            master: Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "compressor".to_string(),
                    params: BTreeMap::from([
                        ("thresholdDb".to_string(), json!(-40.0)),
                        ("ratio".to_string(), json!(12.0)),
                        ("attackMs".to_string(), json!(1.0)),
                        ("releaseMs".to_string(), json!(40.0)),
                        ("makeupDb".to_string(), json!(0.0)),
                        ("mixPct".to_string(), json!(100.0)),
                    ]),
                }],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });

    right_only.note_on(1, 60, 127, 500);
    linked.note_on(0, 60, 127, 500);
    linked.note_on(1, 60, 127, 500);
    let mut right_only_sum = 0.0_f32;
    let mut linked_right_sum = 0.0_f32;
    for _ in 0..4096 {
        let (_, right_only_right) = right_only.next_stereo_sample();
        let (_, linked_right) = linked.next_stereo_sample();
        right_only_sum += right_only_right.abs();
        linked_right_sum += linked_right.abs();
    }

    assert!(linked_right_sum < right_only_sum * 0.75);
}

#[test]
fn set_instruments_preserves_unchanged_master_fx_state() {
    let mut engine = SynthEngine::new(48_000);
    let cfg = default_synth_config();
    let mut changed = cfg;
    changed.filter.cutoff_hz = 1_800.0;
    let master = Some(MasterFxConfig {
        slots: vec![FxBusSlotConfig::Config {
            kind: "compressor".to_string(),
            params: BTreeMap::from([
                ("thresholdDb".to_string(), json!(-30.0)),
                ("ratio".to_string(), json!(8.0)),
                ("attackMs".to_string(), json!(2.0)),
                ("releaseMs".to_string(), json!(80.0)),
                ("makeupDb".to_string(), json!(0.0)),
                ("mixPct".to_string(), json!(100.0)),
            ]),
        }],
    });
    let instruments = |slot_cfg| {
        vec![InstrumentSlotConfig {
            kind: "synth".to_string(),
            synth: slot_cfg,
            mixer: Some(InstrumentMixerConfig {
                route: "direct".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }]
    };
    engine.set_instruments(InstrumentsConfig {
        instruments: instruments(cfg),
        mixer: Some(MixerConfig {
            buses: vec![],
            master: master.clone(),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.note_on(0, 60, 127, 500);
    for _ in 0..256 {
        let _ = engine.next_stereo_sample();
    }
    let before = engine
        .master_compressor_env_probe(0)
        .expect("expected master compressor state");
    assert!(before > 0.0);

    engine.set_instruments(InstrumentsConfig {
        instruments: instruments(changed),
        mixer: Some(MixerConfig {
            buses: vec![],
            master,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let after = engine
        .master_compressor_env_probe(0)
        .expect("expected master compressor state");
    assert_eq!(after, before);
}
