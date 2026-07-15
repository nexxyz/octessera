use super::*;

#[test]
fn silent_fx_bus_stays_idle_without_signal() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(delay_bus_test_config(0.0));

    let before = engine
        .delay_state_probe(0, 0)
        .expect("expected delay state");
    for _ in 0..256 {
        let _ = engine.next_stereo_sample();
    }
    let after = engine
        .delay_state_probe(0, 0)
        .expect("expected delay state");

    assert_eq!(before.0, after.0);
    assert_eq!(before.1, after.1);
}

#[test]
fn active_fx_bus_keeps_processing_after_recent_signal() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(delay_bus_test_config(0.5));

    engine.note_on(0, 60, 100, 10_000);
    let before = engine
        .delay_state_probe(0, 0)
        .expect("expected delay state");
    for _ in 0..128 {
        let _ = engine.next_stereo_sample();
    }
    engine.note_off(0, 60);
    for _ in 0..128 {
        let _ = engine.next_stereo_sample();
    }
    let after = engine
        .delay_state_probe(0, 0)
        .expect("expected delay state");

    assert!(after.0 != before.0 || after.1 > before.1);
    assert!(after.1 > 0.0);
}

fn delay_bus_test_config(feedback: f32) -> InstrumentsConfig {
    InstrumentsConfig {
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
                slots: vec![FxBusSlotConfig::Config {
                    kind: "delay".to_string(),
                    params: BTreeMap::from([
                        ("timeMs".to_string(), json!(10.0)),
                        ("feedback".to_string(), json!(feedback)),
                        ("mixPct".to_string(), json!(100.0)),
                    ]),
                }],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}
