use super::super::*;

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
fn sample_filter_cutoff_changes_active_and_preview_voice_output() {
    let mut dry = sample_filter_engine(20_000.0, 20.0);
    let mut filtered = sample_filter_engine(200.0, 20.0);
    dry.note_on(0, 36, 127, 1_000);
    filtered.note_on(0, 36, 127, 1_000);

    let dry_out = dry.next_sample().abs();
    let filtered_out = filtered.next_sample().abs();
    assert!(dry_out.is_finite());
    assert!(filtered_out.is_finite());
    assert!(filtered_out < dry_out * 0.2);

    let buffer = SampleBuffer {
        samples: vec![1.0, 0.0, 0.0, 0.0].into(),
        channels: 1,
        sample_rate: 48_000,
    };
    let mut dry_preview = sample_filter_engine(20_000.0, 20.0);
    let mut filtered_preview = sample_filter_engine(200.0, 20.0);
    dry_preview.preview_sample(0, buffer.clone(), 127);
    filtered_preview.preview_sample(0, buffer, 127);

    let dry_preview_out = dry_preview.next_sample().abs();
    let filtered_preview_out = filtered_preview.next_sample().abs();
    assert!(dry_preview_out.is_finite());
    assert!(filtered_preview_out.is_finite());
    assert!(filtered_preview_out < dry_preview_out * 0.2);
}

#[test]
fn sample_filter_params_update_from_dynamic_control() {
    let mut engine = sample_filter_engine(8_000.0, 20.0);
    engine.set_sample_bank_param(0, "sample.filter.cutoffHz", 1234.0);
    engine.set_sample_bank_param(0, "sample.filter.resonance", 42.0);
    engine.note_on(0, 36, 127, 1_000);

    assert!(engine.next_sample().is_finite());
}

fn sample_filter_engine(cutoff_hz: f32, resonance: f32) -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "sampler".to_string(),
            synth: default_synth_config(),
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut bank = sample_bank(vec![1.0, 0.0, 0.0, 0.0]);
    bank.filter_cutoff_hz = cutoff_hz;
    bank.filter_resonance = resonance;
    engine.set_sample_banks(vec![bank]);
    engine
}
