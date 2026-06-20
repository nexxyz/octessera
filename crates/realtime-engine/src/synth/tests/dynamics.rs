use super::*;

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
