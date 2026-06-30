use super::helpers::*;

#[test]
fn compiles_mixing_fx_params_with_expected_defaults_and_clamps() {
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
