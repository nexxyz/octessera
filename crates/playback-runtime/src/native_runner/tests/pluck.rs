use super::*;

#[test]
fn pluck_defaults_validate_bounds_and_roundtrip_inactive_type_clone_reset() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let defaults = runner.instruments[0].pluck_config.clone();
    assert_eq!(defaults["decayMs"], 1500);
    assert_eq!(defaults["brightnessPct"], 65);
    assert_eq!(defaults["pickPositionPct"], 25);
    assert_eq!(
        defaults["amp"],
        json!({ "gainPct": 80, "velocitySensitivityPct": 100 })
    );
    assert_eq!(
        defaults["ampEnv"],
        json!({ "attackMs": 0, "decayMs": 0, "sustainPct": 100, "releaseMs": 900 })
    );
    assert_eq!(
        defaults["filter"],
        runner.instruments[0].synth_config["filter"]
    );
    assert_eq!(
        defaults["filterEnv"],
        runner.instruments[0].synth_config["filterEnv"]
    );

    let initial = runner.config_payload();
    for (field, bad) in [
        ("decayMs", json!(99)),
        ("decayMs", json!(5001)),
        ("brightnessPct", json!(101)),
        ("pickPositionPct", json!(4)),
        ("pickPositionPct", json!(51)),
    ] {
        let mut payload = initial.clone();
        payload["runtimeConfig"]["instruments"][0]["pluck"][field] = bad;
        assert!(validate_config_payload(&payload).is_err(), "{field}");
    }
    let mut edited = initial;
    edited["runtimeConfig"]["instruments"][0]["type"] = json!("pluck");
    edited["runtimeConfig"]["instruments"][0]["pluck"]["decayMs"] = json!(2345);
    edited["runtimeConfig"]["instruments"][0]["pluck"]["brightnessPct"] = json!(86);
    edited["runtimeConfig"]["instruments"][0]["pluck"]["pickPositionPct"] = json!(32);
    edited["runtimeConfig"]["instruments"][0]["mixer"]["route"] = json!("fx_bus_2");
    runner.apply_config_payload(edited).unwrap();
    let pluck = runner.instruments[0].pluck_config.clone();
    assert_eq!(runner.instrument_audio_config(0).unwrap()["pluck"], pluck);
    assert_eq!(
        runner.instrument_audio_config(0).unwrap()["mixer"]["route"],
        "fx_bus_2"
    );
    let saved = runner.config_payload();
    assert_eq!(
        RuntimeConfigDto::from_value(&saved["runtimeConfig"])
            .unwrap()
            .to_value()
            .unwrap()["instruments"][0]["pluck"],
        pluck
    );
    let mut synth = saved.clone();
    synth["runtimeConfig"]["instruments"][0]["type"] = json!("synth");
    runner.apply_config_payload(synth).unwrap();
    assert_eq!(runner.instruments[0].pluck_config, pluck);
    runner.apply_config_payload(saved).unwrap();
    assert_eq!(runner.instruments[0].kind, "pluck");
    assert_eq!(runner.instruments[0].pluck_config, pluck);
    runner.instruments[1] = NativeInstrumentSlot::reset(1);
    runner
        .execute_menu_action(NativeMenuAction::CloneInstrument { index: 0 })
        .unwrap();
    confirm_current_dialog(&mut runner);
    assert_eq!(runner.instruments[1].kind, "pluck");
    assert_eq!(runner.instruments[1].pluck_config, pluck);
    runner
        .execute_menu_action(NativeMenuAction::ResetInstrument { index: 1 })
        .unwrap();
    confirm_current_dialog(&mut runner);
    assert_eq!(runner.instruments[1].pluck_config, defaults);
}

#[test]
fn pluck_legacy_full_defaults_and_partial_patch_preserves_existing() {
    for unversioned in [false, true] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let defaults = runner.instruments[0].pluck_config.clone();
        let mut legacy = runner.config_payload();
        for slot in legacy["runtimeConfig"]["instruments"]
            .as_array_mut()
            .unwrap()
        {
            slot.as_object_mut().unwrap().remove("pluck");
        }
        legacy["runtimeConfig"]["instruments"][0]["type"] = json!("pluck");
        let mut edited = runner.config_payload();
        edited["runtimeConfig"]["instruments"][0]["pluck"]["decayMs"] = json!(3200);
        runner.apply_config_payload(edited).unwrap();
        runner
            .apply_patch_payload_preserving_device(json!({
                "runtimeConfig": { "instruments": [{ "type": "pluck" }] }
            }))
            .unwrap();
        assert_eq!(runner.instruments[0].pluck_config["decayMs"], 3200);
        super::super::apply_payload_instrument_values::apply_instrument_pluck_payload(
            &json!({ "type": "synth" }),
            &mut runner.instruments[0],
        );
        assert_eq!(runner.instruments[0].pluck_config["decayMs"], 3200);
        runner
            .apply_config_payload(if unversioned {
                unversioned_payload(legacy)
            } else {
                legacy
            })
            .unwrap();
        assert_eq!(runner.instruments[0].pluck_config, defaults);
        assert_eq!(runner.config_payload()["schemaVersion"], 2);
    }
}

#[test]
fn pluck_portable_patch_accepts_legacy_template_and_rejects_unknown_nested_field() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut edited = runner.config_payload();
    edited["runtimeConfig"]["instruments"][0]["type"] = json!("pluck");
    edited["runtimeConfig"]["instruments"][0]["pluck"]["decayMs"] = json!(2200);
    runner.apply_config_payload(edited).unwrap();
    let patch = portable_patch_projection(&runner.config_payload()).unwrap();
    let mut old = runner.config_payload();
    for slot in old["runtimeConfig"]["instruments"].as_array_mut().unwrap() {
        slot.as_object_mut().unwrap().remove("pluck");
    }
    let loaded = prepare_patch_payload(patch.clone(), &old).unwrap();
    assert_eq!(
        loaded.payload["runtimeConfig"]["instruments"][0]["pluck"]["decayMs"],
        2200
    );
    assert_eq!(portable_patch_projection(&loaded.payload).unwrap(), patch);
    let mut bad = patch.clone();
    bad["runtimeConfig"]["instruments"][0]["pluck"]["futureField"] = json!(true);
    let error = prepare_patch_payload(bad, &old).unwrap_err();
    assert!(
        error.contains("$.runtimeConfig.instruments[0].pluck.futureField"),
        "{error}"
    );
    let mut legacy = patch;
    for slot in legacy["runtimeConfig"]["instruments"]
        .as_array_mut()
        .unwrap()
    {
        slot.as_object_mut().unwrap().remove("pluck");
    }
    prepare_patch_payload(legacy.clone(), &old).unwrap();
    let mut fresh = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    fresh.apply_patch_payload_preserving_device(legacy).unwrap();
    assert_eq!(
        fresh.instruments[0].pluck_config,
        super::super::synth_config::pluck_default_config()
    );
}
