use super::*;

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
                    volume_pct: 100.0,
                },
                FxBusConfig {
                    slots: vec![FxBusSlotConfig::Kind("saturator".to_string())],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume_pct: 100.0,
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
