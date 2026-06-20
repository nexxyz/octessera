use super::super::fx::{
    fx_bus_state_from_params, fx_bus_state_matches_params, master_fx_state_from_params,
    master_fx_state_matches_params, process_fx_bus_slot, process_master_fx_slot, FxBusState,
    MasterFxState,
};
use super::super::fx_params::{compile_fx_bus_params, DuckSource, FilterLfoKind, FxBusParams};
use super::*;
use serde_json::json;
use std::collections::BTreeMap;

fn assert_close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1.0e-6, "expected {expected}, got {actual}");
}

fn fx_config(kind: &str, params: BTreeMap<String, serde_json::Value>) -> FxBusSlotConfig {
    FxBusSlotConfig::Config {
        kind: kind.to_string(),
        params,
    }
}

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

    match compile_fx_bus_params(&fx_config(
        "duck",
        BTreeMap::from([
            ("source".to_string(), json!("B2")),
            ("threshold".to_string(), json!(0.5)),
            ("amountPct".to_string(), json!(125.0)),
            ("attackMs".to_string(), json!(0.01)),
            ("releaseMs".to_string(), json!(5_000.0)),
        ]),
    )) {
        FxBusParams::Duck {
            source,
            threshold,
            amount,
            attack_ms,
            release_ms,
        } => {
            assert!(matches!(source, DuckSource::Bus(1)));
            assert_close(threshold, 0.5);
            assert_close(amount, 1.0);
            assert_close(attack_ms, 0.1);
            assert_close(release_ms, 2_000.0);
        }
        _ => panic!("expected duck params"),
    }

    match compile_fx_bus_params(&fx_config(
        "saturator",
        BTreeMap::from([
            ("drive".to_string(), json!(25.0)),
            ("mixPct".to_string(), json!(50.0)),
        ]),
    )) {
        FxBusParams::Saturator { drive, mix } => {
            assert_close(drive, 20.0);
            assert_close(mix, 0.5);
        }
        _ => panic!("expected saturator params"),
    }

    match compile_fx_bus_params(&fx_config(
        "distortion",
        BTreeMap::from([
            ("drive".to_string(), json!(60.0)),
            ("clip".to_string(), json!(0.01)),
            ("mixPct".to_string(), json!(80.0)),
        ]),
    )) {
        FxBusParams::Distortion { drive, clip, mix } => {
            assert_close(drive, 50.0);
            assert_close(clip, 0.05);
            assert_close(mix, 0.8);
        }
        _ => panic!("expected distortion params"),
    }

    match compile_fx_bus_params(&fx_config(
        "bitcrusher",
        BTreeMap::from([
            ("rateDiv".to_string(), json!(0.4)),
            ("bits".to_string(), json!(18.0)),
            ("mixPct".to_string(), json!(60.0)),
        ]),
    )) {
        FxBusParams::Bitcrusher {
            rate_div,
            bits,
            mix,
        } => {
            assert_eq!(rate_div, 1);
            assert_eq!(bits, 16);
            assert_close(mix, 0.6);
        }
        _ => panic!("expected bitcrusher params"),
    }

    match compile_fx_bus_params(&fx_config(
        "compressor",
        BTreeMap::from([
            ("thresholdDb".to_string(), json!(-70.0)),
            ("ratio".to_string(), json!(30.0)),
            ("attackMs".to_string(), json!(0.01)),
            ("releaseMs".to_string(), json!(5_000.0)),
            ("makeupDb".to_string(), json!(30.0)),
            ("mixPct".to_string(), json!(90.0)),
        ]),
    )) {
        FxBusParams::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_db,
            mix,
        } => {
            assert_close(threshold_db, -60.0);
            assert_close(ratio, 20.0);
            assert_close(attack_ms, 0.1);
            assert_close(release_ms, 2_000.0);
            assert_close(makeup_db, 24.0);
            assert_close(mix, 0.9);
        }
        _ => panic!("expected compressor params"),
    }

    match compile_fx_bus_params(&fx_config(
        "eq",
        BTreeMap::from([
            ("lowGainDb".to_string(), json!(15.0)),
            ("midGainDb".to_string(), json!(-15.0)),
            ("midFreqHz".to_string(), json!(20_000.0)),
            ("midQ".to_string(), json!(0.1)),
            ("highGainDb".to_string(), json!(6.0)),
            ("mixPct".to_string(), json!(40.0)),
        ]),
    )) {
        FxBusParams::Eq {
            low_gain_db,
            mid_gain_db,
            mid_freq_hz,
            mid_q,
            high_gain_db,
            mix,
        } => {
            assert_close(low_gain_db, 12.0);
            assert_close(mid_gain_db, -12.0);
            assert_close(mid_freq_hz, 8_000.0);
            assert_close(mid_q, 0.25);
            assert_close(high_gain_db, 6.0);
            assert_close(mix, 0.4);
        }
        _ => panic!("expected eq params"),
    }

    match compile_fx_bus_params(&fx_config(
        "vinyl",
        BTreeMap::from([
            ("saturationPct".to_string(), json!(25.0)),
            ("cracklePct".to_string(), json!(10.0)),
            ("warpDepthPct".to_string(), json!(15.0)),
            ("mixPct".to_string(), json!(70.0)),
        ]),
    )) {
        FxBusParams::Vinyl {
            saturation,
            crackle,
            warp_depth,
            mix,
        } => {
            assert_close(saturation, 0.25);
            assert_close(crackle, 0.1);
            assert_close(warp_depth, 0.15);
            assert_close(mix, 0.7);
        }
        _ => panic!("expected vinyl params"),
    }

    assert!(matches!(
        compile_fx_bus_params(&fx_config(
            "duck",
            BTreeMap::from([("source".to_string(), json!("wat"))]),
        )),
        FxBusParams::Duck {
            source: DuckSource::Instrument(0),
            ..
        }
    ));
    assert!(matches!(
        compile_fx_bus_params(&FxBusSlotConfig::Kind("unknown".to_string())),
        FxBusParams::None
    ));
}

#[test]
fn fx_state_factories_and_matchers_cover_supported_variants() {
    let params = [
        FxBusParams::None,
        FxBusParams::Tremolo {
            rate_hz: 2.0,
            depth: 0.5,
        },
        FxBusParams::Delay {
            time_ms: 20.0,
            feedback: 0.2,
            mix: 0.3,
        },
        FxBusParams::ModDelay {
            rate_hz: 0.5,
            depth_ms: 3.0,
            base_ms: 8.0,
            feedback: 0.0,
            mix: 0.2,
        },
        FxBusParams::FilterLfo {
            kind: FilterLfoKind::FilterLfo,
            rate_hz: 0.5,
            depth: 0.7,
            center_hz: 1_000.0,
            q: 0.8,
        },
        FxBusParams::Reverb {
            mix: 0.4,
            decay: 0.8,
            damp: 0.2,
        },
        FxBusParams::Glitch {
            chance: 1.0,
            slice_ms: 10.0,
            mix: 0.6,
        },
        FxBusParams::AutoPan {
            rate_hz: 0.5,
            depth: 1.0,
        },
        FxBusParams::Duck {
            source: DuckSource::Instrument(0),
            threshold: 0.1,
            amount: 0.5,
            attack_ms: 10.0,
            release_ms: 50.0,
        },
        FxBusParams::Saturator {
            drive: 2.0,
            mix: 1.0,
        },
        FxBusParams::Distortion {
            drive: 3.0,
            clip: 0.4,
            mix: 0.7,
        },
        FxBusParams::Bitcrusher {
            rate_div: 4,
            bits: 6,
            mix: 0.5,
        },
        FxBusParams::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 80.0,
            makeup_db: 2.0,
            mix: 0.9,
        },
        FxBusParams::Eq {
            low_gain_db: 3.0,
            mid_gain_db: -2.0,
            mid_freq_hz: 1_200.0,
            mid_q: 1.1,
            high_gain_db: 4.0,
            mix: 1.0,
        },
        FxBusParams::Vinyl {
            saturation: 0.2,
            crackle: 0.1,
            warp_depth: 0.3,
            mix: 0.8,
        },
    ];

    for param in params {
        let state = fx_bus_state_from_params(&param, 48_000);
        assert!(fx_bus_state_matches_params(&state, &param));
    }

    let master_params = [
        FxBusParams::None,
        FxBusParams::Saturator {
            drive: 2.0,
            mix: 1.0,
        },
        FxBusParams::Distortion {
            drive: 3.0,
            clip: 0.4,
            mix: 0.7,
        },
        FxBusParams::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 80.0,
            makeup_db: 2.0,
            mix: 0.9,
        },
        FxBusParams::Eq {
            low_gain_db: 3.0,
            mid_gain_db: -2.0,
            mid_freq_hz: 1_200.0,
            mid_q: 1.1,
            high_gain_db: 4.0,
            mix: 1.0,
        },
        FxBusParams::Vinyl {
            saturation: 0.2,
            crackle: 0.1,
            warp_depth: 0.3,
            mix: 0.8,
        },
    ];

    for param in master_params {
        let state = master_fx_state_from_params(&param);
        assert!(master_fx_state_matches_params(&state, &param));
    }

    assert!(!fx_bus_state_matches_params(
        &FxBusState::Tremolo { phase: 0.0 },
        &FxBusParams::Delay {
            time_ms: 10.0,
            feedback: 0.0,
            mix: 0.0,
        },
    ));
    assert!(!master_fx_state_matches_params(
        &MasterFxState::Compressor { env: 0.0 },
        &FxBusParams::Eq {
            low_gain_db: 0.0,
            mid_gain_db: 0.0,
            mid_freq_hz: 1_000.0,
            mid_q: 1.0,
            high_gain_db: 0.0,
            mix: 1.0,
        },
    ));
}

#[test]
fn process_fx_paths_stay_finite_across_bus_and_master_slots() {
    let slot_out = [0.75; INSTRUMENT_SLOT_COUNT];
    let bus_in = [0.5, 0.25];
    let bus_params = [
        FxBusParams::None,
        FxBusParams::Tremolo {
            rate_hz: 2.0,
            depth: 0.8,
        },
        FxBusParams::Delay {
            time_ms: 5.0,
            feedback: 0.3,
            mix: 0.5,
        },
        FxBusParams::ModDelay {
            rate_hz: 0.7,
            depth_ms: 4.0,
            base_ms: 8.0,
            feedback: 0.2,
            mix: 0.5,
        },
        FxBusParams::FilterLfo {
            kind: FilterLfoKind::Wah,
            rate_hz: 1.2,
            depth: 0.7,
            center_hz: 900.0,
            q: 6.0,
        },
        FxBusParams::Reverb {
            mix: 0.4,
            decay: 0.7,
            damp: 0.2,
        },
        FxBusParams::Glitch {
            chance: 1.0,
            slice_ms: 8.0,
            mix: 1.0,
        },
        FxBusParams::AutoPan {
            rate_hz: 0.5,
            depth: 1.0,
        },
        FxBusParams::Duck {
            source: DuckSource::Bus(0),
            threshold: 0.1,
            amount: 0.6,
            attack_ms: 5.0,
            release_ms: 20.0,
        },
        FxBusParams::Saturator {
            drive: 2.0,
            mix: 1.0,
        },
        FxBusParams::Distortion {
            drive: 3.0,
            clip: 0.4,
            mix: 0.7,
        },
        FxBusParams::Bitcrusher {
            rate_div: 4,
            bits: 6,
            mix: 0.5,
        },
        FxBusParams::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 80.0,
            makeup_db: 2.0,
            mix: 0.9,
        },
        FxBusParams::Eq {
            low_gain_db: 3.0,
            mid_gain_db: -2.0,
            mid_freq_hz: 1_200.0,
            mid_q: 1.1,
            high_gain_db: 4.0,
            mix: 1.0,
        },
        FxBusParams::Vinyl {
            saturation: 0.2,
            crackle: 0.1,
            warp_depth: 0.3,
            mix: 0.8,
        },
    ];

    for param in bus_params {
        let mut state = FxBusState::None;
        let first = process_fx_bus_slot(&param, &mut state, 0.5, &slot_out, &bus_in, 48_000);
        let second = process_fx_bus_slot(&param, &mut state, 0.25, &slot_out, &bus_in, 48_000);
        assert!(first.is_finite());
        assert!(second.is_finite());
    }

    let master_params = [
        FxBusParams::None,
        FxBusParams::Saturator {
            drive: 2.0,
            mix: 1.0,
        },
        FxBusParams::Distortion {
            drive: 3.0,
            clip: 0.4,
            mix: 0.7,
        },
        FxBusParams::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 80.0,
            makeup_db: 2.0,
            mix: 0.9,
        },
        FxBusParams::Eq {
            low_gain_db: 3.0,
            mid_gain_db: -2.0,
            mid_freq_hz: 1_200.0,
            mid_q: 1.1,
            high_gain_db: 4.0,
            mix: 1.0,
        },
        FxBusParams::Vinyl {
            saturation: 0.2,
            crackle: 0.1,
            warp_depth: 0.3,
            mix: 0.8,
        },
    ];

    for param in master_params {
        let mut state = MasterFxState::None;
        let first = process_master_fx_slot(&param, &mut state, 0.5, -0.25, 48_000);
        let second = process_master_fx_slot(&param, &mut state, 0.25, -0.5, 48_000);
        assert!(first.0.is_finite() && first.1.is_finite());
        assert!(second.0.is_finite() && second.1.is_finite());
    }
}
