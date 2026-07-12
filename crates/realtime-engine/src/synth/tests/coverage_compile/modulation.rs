use super::param_fixtures::*;

#[test]
fn compiles_fx_bus_params_with_expected_defaults_and_clamps() {
    match compile_fx_bus_params(&fx_config(
        "tremolo",
        BTreeMap::from([
            ("rateHz".to_string(), json!(99.0)),
            ("depthPct".to_string(), json!(150.0)),
        ]),
    )) {
        FxBusParams::Tremolo { rate_hz, depth } => {
            assert_close(rate_hz, 40.0);
            assert_close(depth, 1.0);
        }
        _ => panic!("expected tremolo params"),
    }

    match compile_fx_bus_params(&fx_config(
        "delay",
        BTreeMap::from([
            ("timeMs".to_string(), json!(5_000.0)),
            ("feedback".to_string(), json!(5.0)),
            ("mixPct".to_string(), json!(-5.0)),
        ]),
    )) {
        FxBusParams::Delay {
            time_ms,
            feedback,
            mix,
        } => {
            assert_close(time_ms, 2_000.0);
            assert_close(feedback, 0.98);
            assert_close(mix, 0.0);
        }
        _ => panic!("expected delay params"),
    }

    match compile_fx_bus_params(&fx_config("vibrato", BTreeMap::new())) {
        FxBusParams::ModDelay {
            depth_ms,
            base_ms,
            feedback,
            mix,
            ..
        } => {
            assert_close(depth_ms, 6.0);
            assert_close(base_ms, 8.0);
            assert_close(feedback, 0.0);
            assert_close(mix, 1.0);
        }
        _ => panic!("expected vibrato mod delay params"),
    }

    match compile_fx_bus_params(&fx_config("chorus", BTreeMap::new())) {
        FxBusParams::ModDelay {
            depth_ms,
            base_ms,
            feedback,
            mix,
            ..
        } => {
            assert_close(depth_ms, 14.0);
            assert_close(base_ms, 22.0);
            assert_close(feedback, 0.0);
            assert_close(mix, 0.45);
        }
        _ => panic!("expected chorus mod delay params"),
    }

    match compile_fx_bus_params(&fx_config("flanger", BTreeMap::new())) {
        FxBusParams::ModDelay {
            depth_ms,
            base_ms,
            feedback,
            mix,
            ..
        } => {
            assert_close(depth_ms, 2.0);
            assert_close(base_ms, 3.0);
            assert_close(feedback, 0.35);
            assert_close(mix, 0.45);
        }
        _ => panic!("expected flanger mod delay params"),
    }

    match compile_fx_bus_params(&fx_config(
        "filter_lfo",
        BTreeMap::from([
            ("rateHz".to_string(), json!(0.001)),
            ("depthPct".to_string(), json!(35.0)),
            ("centerHz".to_string(), json!(30_000.0)),
            ("q".to_string(), json!(0.1)),
        ]),
    )) {
        FxBusParams::FilterLfo {
            kind,
            rate_hz,
            depth,
            center_hz,
            q,
        } => {
            assert!(matches!(kind, FilterLfoKind::FilterLfo));
            assert_close(rate_hz, 0.02);
            assert_close(depth, 0.35);
            assert_close(center_hz, 12_000.0);
            assert_close(q, 0.25);
        }
        _ => panic!("expected filter lfo params"),
    }

    match compile_fx_bus_params(&fx_config("wah", BTreeMap::new())) {
        FxBusParams::FilterLfo {
            kind,
            rate_hz,
            center_hz,
            q,
            ..
        } => {
            assert!(matches!(kind, FilterLfoKind::Wah));
            assert_close(rate_hz, 1.2);
            assert_close(center_hz, 900.0);
            assert_close(q, 6.0);
        }
        _ => panic!("expected wah params"),
    }

    match compile_fx_bus_params(&fx_config(
        "reverb",
        BTreeMap::from([
            ("mixPct".to_string(), json!(55.0)),
            ("decay".to_string(), json!(2.0)),
            ("damp".to_string(), json!(-1.0)),
        ]),
    )) {
        FxBusParams::Reverb { mix, decay, damp } => {
            assert_close(mix, 0.55);
            assert_close(decay, 0.995);
            assert_close(damp, 0.0);
        }
        _ => panic!("expected reverb params"),
    }

    match compile_fx_bus_params(&fx_config(
        "glitch",
        BTreeMap::from([
            ("chancePct".to_string(), json!(25.0)),
            ("sliceMs".to_string(), json!(1.0)),
            ("mixPct".to_string(), json!(150.0)),
        ]),
    )) {
        FxBusParams::Glitch {
            chance,
            slice_ms,
            mix,
        } => {
            assert_close(chance, 0.25);
            assert_close(slice_ms, 5.0);
            assert_close(mix, 1.0);
        }
        _ => panic!("expected glitch params"),
    }

    match compile_fx_bus_params(&fx_config(
        "auto_pan",
        BTreeMap::from([
            ("rateHz".to_string(), json!(40.0)),
            ("depthPct".to_string(), json!(75.0)),
        ]),
    )) {
        FxBusParams::AutoPan { rate_hz, depth } => {
            assert_close(rate_hz, 20.0);
            assert_close(depth, 0.75);
        }
        _ => panic!("expected auto pan params"),
    }
}
