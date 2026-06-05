use super::engine::FREEZE_INJECT_MS;
use super::{
    default_synth_config, FilterType, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig, MixerConfig, MomentaryFxTarget,
    SampleBankConfig, SampleBuffer, SampleSlotConfig, SynthEngine, DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn generates_samples() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 100, 120);
    let mut any = false;
    for _ in 0..1024 {
        let s = engine.next_sample();
        if s != 0.0 {
            any = true;
            break;
        }
    }
    assert!(any);
}

#[test]
fn applies_instrument_config() {
    let mut engine = SynthEngine::new(48_000);
    let cfg = default_synth_config();
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".to_string(),
            synth: cfg,
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.note_on(0, 60, 100, 120);
    let s = engine.next_sample();
    assert!(s.is_finite());
}

#[test]
fn mixer_volume_controls_synth_output() {
    let mut muted = SynthEngine::new(48_000);
    muted.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".to_string(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "direct".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 0.0,
            }),
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut full = SynthEngine::new(48_000);
    full.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".to_string(),
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
    muted.note_on(0, 60, 127, 500);
    full.note_on(0, 60, 127, 500);
    let mut muted_sum = 0.0_f32;
    let mut full_sum = 0.0_f32;
    for _ in 0..2048 {
        muted_sum += muted.next_sample().abs();
        full_sum += full.next_sample().abs();
    }
    assert!(muted_sum < full_sum * 0.01);
    assert!(full_sum > 0.0);
}

#[test]
fn mixer_pan_controls_synth_output() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".to_string(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "direct".to_string(),
                pan_pos: 0,
                volume: 100.0,
            }),
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.note_on(0, 60, 127, 500);
    let mut left_sum = 0.0_f32;
    let mut right_sum = 0.0_f32;
    for _ in 0..2048 {
        let (left, right) = engine.next_stereo_sample();
        left_sum += left.abs();
        right_sum += right.abs();
    }
    assert!(left_sum > right_sum * 10.0);
}

#[test]
fn routes_through_dynamic_bus_count_without_allocating_bus_vec() {
    let mut engine = SynthEngine::new(48_000);
    let cfg = default_synth_config();
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".to_string(),
            synth: cfg,
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_2".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![
                FxBusConfig {
                    slots: vec![FxBusSlotConfig::Kind("none".to_string())],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                },
                FxBusConfig {
                    slots: vec![FxBusSlotConfig::Kind("saturator".to_string())],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                },
            ],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.note_on(0, 60, 100, 120);
    for _ in 0..1024 {
        let (left, right) = engine.next_stereo_sample();
        assert!(left.is_finite());
        assert!(right.is_finite());
    }
}

#[test]
fn set_instruments_preserves_unchanged_fx_state() {
    let mut engine = SynthEngine::new(48_000);
    let cfg = default_synth_config();
    let mut changed = cfg;
    changed.filter.cutoff_hz = 3_200.0;
    let mixer = Some(MixerConfig {
        buses: vec![FxBusConfig {
            slots: vec![FxBusSlotConfig::Config {
                kind: "delay".to_string(),
                params: BTreeMap::from([
                    ("timeMs".to_string(), json!(40.0)),
                    ("feedback".to_string(), json!(0.35)),
                    ("mixPct".to_string(), json!(50.0)),
                ]),
            }],
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
        }],
        master: None,
    });
    let instruments = |slot1| {
        vec![
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: cfg,
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".to_string(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            },
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: slot1,
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            },
        ]
    };
    engine.set_instruments(InstrumentsConfig {
        instruments: instruments(cfg),
        mixer: mixer.clone(),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.note_on(0, 60, 100, 500);
    for _ in 0..128 {
        let _ = engine.next_stereo_sample();
    }
    let before = engine
        .delay_state_probe(0, 0)
        .expect("expected delay state");
    assert!(before.1 > 0.0);

    engine.set_instruments(InstrumentsConfig {
        instruments: instruments(changed),
        mixer,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let after = engine
        .delay_state_probe(0, 0)
        .expect("expected delay state");
    assert_eq!(after.0, before.0);
    assert_eq!(after.1, before.1);
}

#[test]
fn sample_instrument_routes_through_bus_fx_delay_tail() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "sampler".to_string(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "delay".to_string(),
                    params: BTreeMap::from([
                        ("timeMs".to_string(), serde_json::json!(1.0)),
                        ("feedback".to_string(), serde_json::json!(0.0)),
                        ("mixPct".to_string(), serde_json::json!(100.0)),
                    ]),
                }],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.set_sample_banks(vec![sample_bank(vec![1.0, 0.0, 0.0, 0.0])]);
    engine.note_on(0, 36, 127, 1_000);

    for _ in 0..47 {
        let _ = engine.next_stereo_sample();
    }
    let before_tail = engine.next_sample().abs();
    let tail = engine.next_sample().abs();

    assert!(before_tail < 1.0e-6);
    assert!(
        tail > 0.1,
        "sample routed through delay bus should produce a delayed tail"
    );
}

#[test]
fn sample_preview_routes_through_bus_fx_delay_tail() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "sampler".to_string(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "delay".to_string(),
                    params: BTreeMap::from([
                        ("timeMs".to_string(), serde_json::json!(1.0)),
                        ("feedback".to_string(), serde_json::json!(0.0)),
                        ("mixPct".to_string(), serde_json::json!(100.0)),
                    ]),
                }],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let buffer = SampleBuffer {
        samples: vec![1.0, 0.0, 0.0, 0.0].into(),
        channels: 1,
        sample_rate: 48_000,
    };
    engine.preview_sample(0, buffer, 127);

    for _ in 0..47 {
        let _ = engine.next_stereo_sample();
    }
    let before_tail = engine.next_sample().abs();
    let tail = engine.next_sample().abs();

    assert!(before_tail < 1.0e-6);
    assert!(
        tail > 0.1,
        "sample preview should route through the delay bus"
    );
}

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

#[test]
fn duck_reduces_target_bus_when_source_instrument_is_active() {
    let mut dry = duck_test_engine(false);
    let mut ducked = duck_test_engine(true);
    dry.note_on(0, 36, 127, 1_000);
    dry.note_on(1, 36, 127, 1_000);
    ducked.note_on(0, 36, 127, 1_000);
    ducked.note_on(1, 36, 127, 1_000);

    let mut dry_sum = 0.0;
    let mut ducked_sum = 0.0;
    for _ in 0..256 {
        dry_sum += dry.next_sample().abs();
        ducked_sum += ducked.next_sample().abs();
    }

    assert!(
        ducked_sum < dry_sum * 0.8,
        "duck FX should audibly attenuate the target bus"
    );
}

#[test]
fn all_filter_types_generate_finite_non_silent_audio() {
    let modes = [
        FilterType::Lowpass,
        FilterType::Highpass,
        FilterType::Bandpass,
        FilterType::Notch,
    ];

    for mode in modes {
        let mut engine = SynthEngine::new(48_000);
        let mut cfg = default_synth_config();
        cfg.filter.kind = mode;
        cfg.filter.cutoff_hz = 2_000.0;
        cfg.filter.resonance = 45.0;

        engine.set_instruments(InstrumentsConfig {
            instruments: vec![InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: cfg,
                mixer: None,
            }],
            mixer: None,
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        });

        engine.note_on(0, 64, 110, 220);
        let mut had_nonzero = false;
        for _ in 0..4096 {
            let s = engine.next_sample();
            assert!(s.is_finite(), "sample must be finite for mode {mode:?}");
            if s.abs() > 1.0e-6 {
                had_nonzero = true;
            }
        }

        assert!(
            had_nonzero,
            "expected non-silent output for filter mode {mode:?}"
        );
    }
}

#[test]
fn momentary_stutter_repeats_initial_capture() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "a".to_string(),
        "stutter".to_string(),
        BTreeMap::from([
            ("rateHz".to_string(), json!(30.0)),
            ("depthPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    let segment_len = (48_000.0 / 30.0) as usize;
    let ramp_len = ((48_000.0 * 0.002) as usize).min(segment_len / 4).max(1);

    let mut captured = Vec::new();
    for _ in 0..segment_len {
        captured.push(engine.next_sample());
    }

    let (_, _, write, ready, _) = engine.stutter_buf_for_id("a").unwrap();
    assert!(ready, "stutter should be ready after capture");
    assert_eq!(write, 0, "stutter write should be 0 after capture");

    for _ in 0..ramp_len {
        let _ = engine.next_sample();
    }

    let mut block_a = Vec::new();
    for _ in 0..segment_len {
        block_a.push(engine.next_sample());
    }
    let mut block_b = Vec::new();
    for _ in 0..segment_len {
        block_b.push(engine.next_sample());
    }

    for (i, (x, y)) in block_a.iter().zip(block_b.iter()).enumerate() {
        let diff = (x - y).abs();
        assert!(
            diff < 1.0e-6,
            "stutter loop mismatch at index {i}: a={x} b={y} diff={diff}"
        );
    }
}

#[test]
fn momentary_stutter_stop_restores_normal_output() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "a".to_string(),
        "stutter".to_string(),
        BTreeMap::from([
            ("rateHz".to_string(), json!(12.0)),
            ("depthPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    let segment_len = (48_000.0 / 12.0) as usize;
    let ramp_len = ((48_000.0 * 0.002) as usize).min(segment_len / 4).max(1);
    for _ in 0..segment_len + ramp_len + 512 {
        let _ = engine.next_sample();
    }

    engine.momentary_fx_stop("a");
    let mut released_sum = 0.0_f32;
    for _ in 0..1024 {
        released_sum += engine.next_sample().abs();
    }
    assert!(
        released_sum > 0.1,
        "stutter stop should restore audio output"
    );
}

#[test]
fn momentary_freeze_injection_creates_sustained_tail() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 127, 10_000);

    engine.momentary_fx_start(
        "f".to_string(),
        "freeze".to_string(),
        BTreeMap::from([("mixPct".to_string(), json!(100.0))]),
        MomentaryFxTarget::Global,
    );

    let inject_samples = 48_000 * FREEZE_INJECT_MS / 1000 + 128;
    for _ in 0..inject_samples {
        let _ = engine.next_sample();
    }

    engine.note_off(0, 60);
    for _ in 0..2048 {
        let _ = engine.next_sample();
    }

    let mut sum = 0.0_f32;
    for _ in 0..2048 {
        sum += engine.next_sample().abs();
    }
    assert!(
        sum > 0.0,
        "freeze should sustain reverb tail after note release: {sum}"
    );
    assert!(sum.is_finite(), "freeze output should be finite");
}

#[test]
fn momentary_freeze_on_silence_stays_quiet() {
    let mut engine = SynthEngine::new(48_000);
    engine.momentary_fx_start(
        "f".to_string(),
        "freeze".to_string(),
        BTreeMap::from([("mixPct".to_string(), json!(100.0))]),
        MomentaryFxTarget::Global,
    );

    let inject_samples = 48_000 * FREEZE_INJECT_MS / 1000 + 128;
    for _ in 0..inject_samples {
        let _ = engine.next_sample();
    }

    engine.note_on(0, 60, 120, 1_000);
    let mut sum = 0.0_f32;
    for _ in 0..2048 {
        sum += engine.next_sample().abs();
    }
    assert!(
        sum < 1.0e-6,
        "freeze should not pass live audio through after injection window: {sum}"
    );
}

#[test]
fn momentary_freeze_release_fades_then_removes() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "f".to_string(),
        "freeze".to_string(),
        BTreeMap::from([
            ("mixPct".to_string(), json!(100.0)),
            ("releaseMs".to_string(), json!(10.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    let inject_samples = 48_000 * FREEZE_INJECT_MS / 1000 + 128;
    for _ in 0..inject_samples {
        let _ = engine.next_sample();
    }

    engine.momentary_fx_stop("f");

    let mut release_sum = 0.0_f32;
    for _ in 0..(10 * 48_000 / 1000 + 64) {
        release_sum += engine.next_sample().abs();
    }

    let mut after_sum = 0.0_f32;
    for _ in 0..512 {
        after_sum += engine.next_sample().abs();
    }

    assert!(release_sum > 0.0, "release tail should produce audio");
    assert!(
        after_sum > 0.1,
        "freeze stop should restore normal audio output: {after_sum}"
    );
}

#[test]
fn momentary_filter_and_pitch_shift_stay_finite() {
    for (fx_type, params) in [
        (
            "filter_sweep",
            BTreeMap::from([
                ("cutoffPct".to_string(), json!(20.0)),
                ("resonancePct".to_string(), json!(80.0)),
            ]),
        ),
        (
            "pitch_shift",
            BTreeMap::from([
                ("semitones".to_string(), json!(7.0)),
                ("mixPct".to_string(), json!(100.0)),
            ]),
        ),
    ] {
        let mut engine = SynthEngine::new(48_000);
        engine.note_on(0, 60, 120, 1_000);
        engine.momentary_fx_start(
            "fx".to_string(),
            fx_type.to_string(),
            params,
            MomentaryFxTarget::Global,
        );
        let mut sum = 0.0_f32;
        for _ in 0..2048 {
            let sample = engine.next_sample();
            assert!(sample.is_finite());
            sum += sample.abs();
        }
        assert!(
            sum > 0.0,
            "{fx_type} should produce non-silent finite output"
        );
    }
}

#[test]
fn momentary_pitch_shift_fills_and_reads_output_buffer() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "ps".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(7.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    assert!(
        engine.pitch_buf_probe("ps").is_some(),
        "pitch shift fx should exist"
    );

    let mut any = false;
    for _ in 0..256 {
        let sample = engine.next_sample();
        if sample != 0.0 {
            any = true;
            break;
        }
    }
    assert!(
        any,
        "pitch shift should produce non-zero output within 256 frames"
    );
}

#[test]
fn momentary_pitch_shift_different_params_produce_different_output() {
    let mut engine_a = SynthEngine::new(48_000);
    engine_a.note_on(0, 60, 120, 1_000);
    engine_a.momentary_fx_start(
        "ps_a".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(3.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    let mut engine_b = SynthEngine::new(48_000);
    engine_b.note_on(0, 60, 120, 1_000);
    engine_b.momentary_fx_start(
        "ps_b".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(4.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    let mut diff = false;
    for _ in 0..8192 {
        let a = engine_a.next_sample();
        let b = engine_b.next_sample();
        if (a - b).abs() > 0.001 {
            diff = true;
            break;
        }
    }
    assert!(
        diff,
        "different semitone values should produce different output"
    );
}

#[test]
fn momentary_pitch_shift_cents_combined_with_semitones() {
    let mut engine_a = SynthEngine::new(48_000);
    engine_a.note_on(0, 60, 120, 1_000);
    engine_a.momentary_fx_start(
        "ps_a".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(5.0)),
            ("cents".to_string(), json!(0.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    let mut engine_b = SynthEngine::new(48_000);
    engine_b.note_on(0, 60, 120, 1_000);
    engine_b.momentary_fx_start(
        "ps_b".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(5.0)),
            ("cents".to_string(), json!(50.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    let mut diff = false;
    for _ in 0..8192 {
        let a = engine_a.next_sample();
        let b = engine_b.next_sample();
        if (a - b).abs() > 0.001 {
            diff = true;
            break;
        }
    }
    assert!(
        diff,
        "same semitones with different cents should produce different output"
    );
}

#[test]
fn momentary_pitch_shift_stop_immediately_removes() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "ps".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(12.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    for _ in 0..128 {
        engine.next_sample();
    }
    assert!(
        engine.pitch_buf_probe("ps").is_some(),
        "pitch shift should exist before stop"
    );
    engine.momentary_fx_stop("ps");
    assert!(
        engine.pitch_buf_probe("ps").is_none(),
        "pitch shift should be immediately removed on stop"
    );
}

#[test]
fn momentary_pitch_shift_no_gap_on_activation() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);

    for _ in 0..1024 {
        engine.next_sample();
    }

    let mut pre_energy = 0.0_f32;
    for _ in 0..128 {
        pre_energy += engine.next_sample().abs();
    }
    pre_energy /= 128.0;
    assert!(
        pre_energy > 0.001,
        "pre-activation energy should be non-trivial: {pre_energy}"
    );

    engine.momentary_fx_start(
        "ps".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(7.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    let mut buf = Vec::with_capacity(512);
    for _ in 0..512 {
        buf.push(engine.next_sample().abs());
    }

    let post_sum: f32 = buf.iter().sum();
    let expected_min = pre_energy * 512.0 * 0.25;
    assert!(
        post_sum > expected_min,
        "pitch shift activation should maintain overall energy: {post_sum} vs {expected_min}"
    );

    let threshold = pre_energy * 0.02;
    let mut max_quiet_run = 0usize;
    let mut quiet_run = 0usize;
    for s in &buf {
        if *s < threshold {
            quiet_run += 1;
            max_quiet_run = max_quiet_run.max(quiet_run);
        } else {
            quiet_run = 0;
        }
    }
    assert!(
        max_quiet_run < 48,
        "pitch shift activation produced a {max_quiet_run}-sample near-silent run"
    );
}

#[test]
fn momentary_filter_sweep_envelope_changes_cutoff_over_time() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "sweep".to_string(),
        "filter_sweep".to_string(),
        BTreeMap::from([
            ("cutoffPct".to_string(), json!(10.0)),
            ("sweepInMs".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    let early = engine.next_sample().abs();
    for _ in 0..(48_000 * 100 / 1000 - 2) {
        let _ = engine.next_sample();
    }
    let late = engine.next_sample().abs();

    assert!(
        (late - early).abs() > 0.001,
        "filter sweep should change output over time: early={early} late={late}"
    );
}

#[test]
fn momentary_filter_sweep_stop_releases_then_removes() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 10_000);
    engine.momentary_fx_start(
        "sweep".to_string(),
        "filter_sweep".to_string(),
        BTreeMap::from([
            ("cutoffPct".to_string(), json!(10.0)),
            ("sweepInMs".to_string(), json!(50.0)),
            ("sweepOutMs".to_string(), json!(10.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    for _ in 0..(48_000 * 50 / 1000) {
        let _ = engine.next_sample();
    }

    engine.momentary_fx_stop("sweep");

    let mut short_release_sum = 0.0_f32;
    for _ in 0..(48_000 * 10 / 1000 + 64) {
        short_release_sum += engine.next_sample().abs();
    }

    assert!(
        short_release_sum > 0.0,
        "sweep release tail should produce audio"
    );
}

#[test]
fn momentary_filter_sweep_stop_with_long_sweep_out() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 10_000);
    engine.momentary_fx_start(
        "sweep".to_string(),
        "filter_sweep".to_string(),
        BTreeMap::from([
            ("cutoffPct".to_string(), json!(10.0)),
            ("sweepInMs".to_string(), json!(10.0)),
            ("sweepOutMs".to_string(), json!(200.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    for _ in 0..(48_000 * 10 / 1000) {
        let _ = engine.next_sample();
    }

    engine.momentary_fx_stop("sweep");

    let mut early_release = 0.0_f32;
    for _ in 0..(48_000 * 10 / 1000) {
        early_release += engine.next_sample().abs();
    }
    let mut late_release = 0.0_f32;
    for _ in 0..(48_000 * 10 / 1000) {
        late_release += engine.next_sample().abs();
    }

    assert!(
        early_release > 0.0 && late_release > 0.0,
        "release tail should persist during long sweep-out"
    );
}

#[test]
fn maintains_eight_voices_per_instrument_slot() {
    let mut engine = SynthEngine::new(48_000);
    for i in 0..8 {
        engine.note_on(0, 60 + i, 100, 2_000);
        engine.note_on(1, 72 + i, 100, 2_000);
    }

    assert_eq!(engine.active_voice_count_for_slot(0), 8);
    assert_eq!(engine.active_voice_count_for_slot(1), 8);
}

#[test]
fn voice_steal_is_scoped_to_instrument_slot() {
    let mut engine = SynthEngine::new(48_000);
    for i in 0..8 {
        engine.note_on(0, 60 + i, 100, 2_000);
        engine.note_on(1, 72 + i, 100, 2_000);
    }
    engine.note_on(0, 90, 100, 2_000);

    assert_eq!(engine.active_voice_count_for_slot(0), 8);
    assert_eq!(engine.active_voice_count_for_slot(1), 8);
}

#[test]
fn note_off_releases_matching_slot_note() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 100, 50_000);
    for _ in 0..64 {
        let _ = engine.next_sample();
    }
    engine.note_off(0, 60);
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}

#[test]
fn all_notes_off_releases_all_slots() {
    let mut engine = SynthEngine::new(48_000);
    for i in 0..4 {
        engine.note_on(0, 60 + i, 100, 50_000);
        engine.note_on(1, 72 + i, 100, 50_000);
    }
    engine.all_notes_off();
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
    assert_eq!(engine.active_voice_count_for_slot(1), 0);
}

#[test]
fn cc_updates_mod_slots_and_reset_cc_clears_them() {
    let mut engine = SynthEngine::new(48_000);
    engine.cc(0, 74, 127);
    engine.cc(0, 71, 64);
    let (cutoff, resonance) = engine.mod_values_for_slot(0);
    assert!(cutoff > 0.99);
    assert!(resonance > 0.49 && resonance < 0.51);

    engine.cc(0, 123, 0);
    let (cutoff_after, resonance_after) = engine.mod_values_for_slot(0);
    assert_eq!(cutoff_after, 0.0);
    assert_eq!(resonance_after, 0.0);
}

#[test]
fn note_on_clamps_slot_and_velocity() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(200, 60, 0, 1_000);
    assert_eq!(
        engine.active_voice_count_for_slot(INSTRUMENT_SLOT_COUNT - 1),
        1
    );
    for _ in 0..100 {
        let s = engine.next_sample();
        assert!(s.is_finite());
    }
}

#[test]
fn zero_duration_note_releases_after_minimum_samples() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 100, 0);
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}

#[test]
fn long_running_event_stream_stays_finite() {
    let mut engine = SynthEngine::new(48_000);
    for i in 0..200 {
        let slot = (i % INSTRUMENT_SLOT_COUNT) as u8;
        let note = 36 + (i % 48) as u8;
        let vel = 1 + (i % 127) as u8;
        engine.note_on(slot, note, vel, 50 + (i % 200) as u32);
        engine.cc(slot, 74, (i % 128) as u8);
        engine.cc(slot, 71, ((i * 3) % 128) as u8);
        if i % 11 == 0 {
            engine.cc(slot, 120, 0);
        }
        for _ in 0..128 {
            let s = engine.next_sample();
            assert!(s.is_finite());
            assert!((-1.0..=1.0).contains(&s));
        }
    }
}

fn sample_bank(samples: Vec<f32>) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: samples.into(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    bank
}

#[test]
fn compressor_quieter_when_above_threshold() {
    let mut dry = SynthEngine::new(48_000);
    dry.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: 4,
                    volume: 100.0,
                }),
            };
            INSTRUMENT_SLOT_COUNT
        ],
        mixer: None,
        pan_positions: 8,
        master_volume: 100.0,
    });

    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".to_string(),
                    pan_pos: 4,
                    volume: 100.0,
                }),
            };
            INSTRUMENT_SLOT_COUNT
        ],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![
                    FxBusSlotConfig::Config {
                        kind: "compressor".to_string(),
                        params: BTreeMap::from([
                            ("thresholdDb".to_string(), json!(-40.0)),
                            ("ratio".to_string(), json!(10.0)),
                            ("attackMs".to_string(), json!(1.0)),
                            ("releaseMs".to_string(), json!(20.0)),
                            ("makeupDb".to_string(), json!(0.0)),
                            ("mixPct".to_string(), json!(100.0)),
                        ]),
                    },
                    FxBusSlotConfig::Kind("none".to_string()),
                ],
                pan_pos: 4,
            }],
            master: None,
        }),
        pan_positions: 8,
        master_volume: 100.0,
    });

    dry.note_on(0, 60, 127, 500);
    engine.note_on(0, 60, 127, 500);
    let mut dry_sum = 0.0_f32;
    let mut comp_sum = 0.0_f32;
    for _ in 0..4096 {
        dry_sum += dry.next_sample().abs();
        comp_sum += engine.next_sample().abs();
    }
    assert!(
        comp_sum < dry_sum * 0.85,
        "compressor should reduce gain: dry={dry_sum} comp={comp_sum}"
    );
    assert!(
        comp_sum > dry_sum * 0.05,
        "compressor should not fully mute: {comp_sum}"
    );
}

#[test]
fn compressor_makeup_restores_gain() {
    let mut dry = SynthEngine::new(48_000);
    dry.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: 4,
                    volume: 100.0,
                }),
            };
            INSTRUMENT_SLOT_COUNT
        ],
        mixer: None,
        pan_positions: 8,
        master_volume: 100.0,
    });

    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".to_string(),
                    pan_pos: 4,
                    volume: 100.0,
                }),
            };
            INSTRUMENT_SLOT_COUNT
        ],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![
                    FxBusSlotConfig::Config {
                        kind: "compressor".to_string(),
                        params: BTreeMap::from([
                            ("thresholdDb".to_string(), json!(-20.0)),
                            ("ratio".to_string(), json!(4.0)),
                            ("attackMs".to_string(), json!(1.0)),
                            ("releaseMs".to_string(), json!(50.0)),
                            ("makeupDb".to_string(), json!(12.0)),
                            ("mixPct".to_string(), json!(100.0)),
                        ]),
                    },
                    FxBusSlotConfig::Kind("none".to_string()),
                ],
                pan_pos: 4,
            }],
            master: None,
        }),
        pan_positions: 8,
        master_volume: 100.0,
    });

    dry.note_on(0, 60, 127, 500);
    engine.note_on(0, 60, 127, 500);
    let mut dry_sum = 0.0_f32;
    let mut comp_sum = 0.0_f32;
    for _ in 0..4096 {
        dry_sum += dry.next_sample().abs();
        comp_sum += engine.next_sample().abs();
    }
    assert!(
        comp_sum > dry_sum * 0.3,
        "makeup gain should restore significant level: dry={dry_sum} comp={comp_sum}"
    );
}

#[test]
fn eq_boosts_and_cuts_band_energy() {
    let mut flat = SynthEngine::new(48_000);
    flat.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: 4,
                    volume: 100.0,
                }),
            };
            INSTRUMENT_SLOT_COUNT
        ],
        mixer: None,
        pan_positions: 8,
        master_volume: 100.0,
    });

    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".to_string(),
                    pan_pos: 4,
                    volume: 100.0,
                }),
            };
            INSTRUMENT_SLOT_COUNT
        ],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![
                    FxBusSlotConfig::Config {
                        kind: "eq".to_string(),
                        params: BTreeMap::from([
                            ("lowGainDb".to_string(), json!(6.0)),
                            ("midGainDb".to_string(), json!(-6.0)),
                            ("midFreqHz".to_string(), json!(1000.0)),
                            ("midQ".to_string(), json!(2.0)),
                            ("highGainDb".to_string(), json!(6.0)),
                            ("mixPct".to_string(), json!(100.0)),
                        ]),
                    },
                    FxBusSlotConfig::Kind("none".to_string()),
                ],
                pan_pos: 4,
            }],
            master: None,
        }),
        pan_positions: 8,
        master_volume: 100.0,
    });

    flat.note_on(0, 60, 127, 1000);
    engine.note_on(0, 60, 127, 1000);
    let mut flat_sum = 0.0_f32;
    let mut eq_sum = 0.0_f32;
    for _ in 0..8192 {
        flat_sum += flat.next_sample().abs();
        eq_sum += engine.next_sample().abs();
    }
    assert!(
        (eq_sum - flat_sum).abs() > flat_sum * 0.02,
        "EQ should measurably change signal energy: flat={flat_sum} eq={eq_sum}"
    );
    assert!(
        eq_sum.is_finite() && eq_sum > 0.0,
        "EQ output should be finite and non-zero: {eq_sum}"
    );
}

fn duck_test_engine(with_duck: bool) -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    let slot1 = if with_duck {
        FxBusSlotConfig::Config {
            kind: "duck".to_string(),
            params: BTreeMap::from([
                ("source".to_string(), serde_json::json!("I1")),
                ("threshold".to_string(), serde_json::json!(0.01)),
                ("amountPct".to_string(), serde_json::json!(100.0)),
                ("attackMs".to_string(), serde_json::json!(1.0)),
                ("releaseMs".to_string(), serde_json::json!(20.0)),
            ]),
        }
    } else {
        FxBusSlotConfig::Kind("none".to_string())
    };
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "sampler".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_2".to_string(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            },
            InstrumentSlotConfig {
                kind: "sampler".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".to_string(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            },
        ],
        mixer: Some(MixerConfig {
            buses: vec![
                FxBusConfig {
                    slots: vec![slot1],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                },
                FxBusConfig {
                    slots: vec![FxBusSlotConfig::Kind("none".to_string())],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                },
            ],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.set_sample_banks(vec![
        sample_bank(vec![1.0; 512]),
        sample_bank(vec![0.5; 512]),
    ]);
    engine
}
