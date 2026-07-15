use super::*;

#[test]
fn fx_active_slot_indices_preserve_none_gaps_and_autopan() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
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
                slots: vec![
                    FxBusSlotConfig::Kind("none".to_string()),
                    FxBusSlotConfig::Config {
                        kind: "auto_pan".to_string(),
                        params: BTreeMap::from([
                            ("rateHz".to_string(), json!(20.0)),
                            ("depthPct".to_string(), json!(100.0)),
                        ]),
                    },
                ],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: Some(MasterFxConfig {
                slots: vec![
                    FxBusSlotConfig::Kind("none".to_string()),
                    FxBusSlotConfig::Config {
                        kind: "compressor".to_string(),
                        params: BTreeMap::from([
                            ("thresholdDb".to_string(), json!(-24.0)),
                            ("ratio".to_string(), json!(4.0)),
                            ("attackMs".to_string(), json!(5.0)),
                            ("releaseMs".to_string(), json!(80.0)),
                            ("makeupDb".to_string(), json!(0.0)),
                            ("mixPct".to_string(), json!(100.0)),
                        ]),
                    },
                ],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.note_on(0, 60, 127, 1_000);

    let mut stereo_difference = 0.0_f32;
    for _ in 0..512 {
        let (left, right) = engine.next_stereo_sample();
        stereo_difference += (left - right).abs();
    }

    assert!(
        stereo_difference > 0.01,
        "auto-pan after a none slot should still affect bus output"
    );
    assert!(
        engine.master_compressor_env_probe(1).unwrap_or_default() > 0.0,
        "master compressor after a none slot should still process output"
    );
}
