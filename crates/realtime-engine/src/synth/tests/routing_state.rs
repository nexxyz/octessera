use super::super::*;

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
