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
