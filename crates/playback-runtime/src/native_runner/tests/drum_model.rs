use super::*;

fn old_drum_config(mut payload: Value) -> Value {
    for slot in payload["runtimeConfig"]["instruments"]
        .as_array_mut()
        .unwrap()
    {
        slot.as_object_mut().unwrap().remove("drum");
    }
    payload
}

#[test]
fn drum_defaults_are_eight_independent_voices_with_empty_assignments_and_own_mixer() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let drum = &runner.instruments[0].drum_config;
    assert_eq!(drum["voices"].as_array().unwrap().len(), 8);
    assert_eq!(drum["assignments"], json!([]));
    for (index, (sound, decay, tone)) in [
        ("kick", 420, 35),
        ("snare", 220, 70),
        ("closed_hat", 85, 90),
        ("open_hat", 650, 85),
        ("low_tom", 500, 50),
        ("high_tom", 320, 60),
        ("clap", 240, 80),
        ("rim", 95, 80),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            drum["voices"][index],
            json!({
                "sound": sound, "tuneSemis": 0, "decayMs": decay, "tonePct": tone, "attackMs": 0,
            })
        );
    }
    assert_eq!(
        drum["amp"],
        json!({ "gainPct": 80, "velocitySensitivityPct": 100 })
    );
    assert_eq!(
        drum["ampEnv"],
        json!({ "attackMs": 0, "decayMs": 0, "sustainPct": 100, "releaseMs": 30 })
    );
    assert_eq!(drum["filter"], runner.instruments[0].synth_config["filter"]);
    assert_eq!(
        drum["filterEnv"],
        runner.instruments[0].synth_config["filterEnv"]
    );
}

#[test]
fn drum_full_legacy_defaults_partial_patch_preserves_and_v2_unknown_field_stays_strict() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let defaults = runner.instruments[0].drum_config.clone();
    let legacy = old_drum_config(runner.config_payload());
    let mut edited = runner.config_payload();
    edited["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    edited["runtimeConfig"]["instruments"][0]["drum"]["voices"][0]["tonePct"] = json!(87);
    runner.apply_config_payload(edited).unwrap();
    runner
        .apply_patch_payload_preserving_device(json!({
            "runtimeConfig": { "instruments": [{ "type": "drum" }] }
        }))
        .unwrap();
    assert_eq!(
        runner.instruments[0].drum_config["voices"][0]["tonePct"],
        87
    );
    runner.apply_config_payload(legacy.clone()).unwrap();
    assert_eq!(runner.instruments[0].drum_config, defaults);
    runner
        .apply_config_payload(unversioned_payload(legacy))
        .unwrap();
    assert_eq!(runner.instruments[0].drum_config, defaults);

    let mut source = runner.config_payload();
    source["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    source["runtimeConfig"]["instruments"][0]["drum"]["voices"][0]["decayMs"] = json!(777);
    runner.apply_config_payload(source).unwrap();
    let patch = portable_patch_projection(&runner.config_payload()).unwrap();
    let old = old_drum_config(runner.config_payload());
    let prepared = prepare_patch_payload(patch.clone(), &old).unwrap();
    assert_eq!(
        prepared.payload["runtimeConfig"]["instruments"][0]["drum"]["voices"][0]["decayMs"],
        777
    );
    assert_eq!(portable_patch_projection(&prepared.payload).unwrap(), patch);
    let mut bad = patch.clone();
    bad["runtimeConfig"]["instruments"][0]["drum"]["futureField"] = json!(true);
    let error = prepare_patch_payload(bad, &old).unwrap_err();
    assert!(
        error.contains("$.runtimeConfig.instruments[0].drum.futureField"),
        "{error}"
    );
    let mut old_patch = patch;
    for slot in old_patch["runtimeConfig"]["instruments"]
        .as_array_mut()
        .unwrap()
    {
        slot.as_object_mut().unwrap().remove("drum");
    }
    prepare_patch_payload(old_patch.clone(), &old).unwrap();
    let mut fresh = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    fresh
        .apply_patch_payload_preserving_device(old_patch)
        .unwrap();
    assert_eq!(fresh.instruments[0].drum_config, defaults);
}

#[test]
fn v2_drum_assignments_restore_against_empty_or_untuned_current_and_reject_unknown_keys() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut edited = source.config_payload();
    edited["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    let assignment = json!({ "x": 3, "y": 0, "voice": 2, "tuneSemis": 24 });
    edited["runtimeConfig"]["instruments"][0]["drum"]["assignments"] = json!([assignment.clone()]);
    source.apply_config_payload(edited).unwrap();
    let patch = portable_patch_projection(&source.config_payload()).unwrap();

    let defaults = NativeRunner::new(NativeRunnerConfig::default())
        .unwrap()
        .config_payload();
    let legacy = old_drum_config(defaults.clone());
    let mut untuned = defaults;
    untuned["runtimeConfig"]["instruments"][0]["drum"]["assignments"] =
        json!([{ "x": 3, "y": 0, "voice": 2 }]);
    for current in [legacy.clone(), untuned] {
        let prepared = prepare_patch_payload(patch.clone(), &current).unwrap();
        assert_eq!(
            prepared.payload["runtimeConfig"]["instruments"][0]["drum"]["assignments"],
            json!([assignment.clone()])
        );
        let mut fresh = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        fresh.apply_config_payload(current).unwrap();
        fresh
            .apply_patch_payload_preserving_device(patch.clone())
            .unwrap();
        assert_eq!(
            fresh.instruments[0].drum_config["assignments"],
            json!([assignment.clone()])
        );
        assert_eq!(fresh.patch_payload().unwrap(), patch);
    }

    let mut unknown = patch.clone();
    unknown["runtimeConfig"]["instruments"][0]["drum"]["assignments"][0]["futureField"] =
        json!(true);
    assert_eq!(
        prepare_patch_payload(unknown, &legacy).unwrap_err(),
        "$.runtimeConfig.instruments[0].drum.assignments[0].futureField is unknown in a v2 portable patch"
    );
    for (assignments, expected) in [
        (
            json!({}),
            "$.runtimeConfig.instruments[0].drum.assignments must be an array",
        ),
        (
            json!([3]),
            "$.runtimeConfig.instruments[0].drum.assignments[0] must be an object",
        ),
    ] {
        let mut malformed = patch.clone();
        malformed["runtimeConfig"]["instruments"][0]["drum"]["assignments"] = assignments;
        assert_eq!(
            prepare_patch_payload(malformed, &legacy).unwrap_err(),
            expected
        );
    }
    for (key, invalid) in [
        ("x", json!(-1)),
        ("y", json!(0.5)),
        ("voice", json!("2")),
        ("tuneSemis", json!("24")),
    ] {
        let mut malformed = patch.clone();
        malformed["runtimeConfig"]["instruments"][0]["drum"]["assignments"][0][key] = invalid;
        let error = prepare_patch_payload(malformed, &legacy).unwrap_err();
        assert!(
            error.contains(&format!(".drum.assignments[0].{key}")),
            "{error}"
        );
    }
}

#[test]
fn drum_config_validation_rejects_out_of_range_duplicate_and_non_eight_voice_banks() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let original = runner.config_payload();
    for (field, bad) in [
        ("tuneSemis", json!(13)),
        ("decayMs", json!(19)),
        ("tonePct", json!(101)),
        ("attackMs", json!(51)),
        ("sound", json!("unknown")),
    ] {
        let mut payload = original.clone();
        payload["runtimeConfig"]["instruments"][0]["drum"]["voices"][0][field] = bad;
        assert!(validate_config_payload(&payload).is_err(), "{field}");
    }
    let mut short = original.clone();
    short["runtimeConfig"]["instruments"][0]["drum"]["voices"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(validate_config_payload(&short).is_err());
    let mut repeated = original;
    repeated["runtimeConfig"]["instruments"][0]["drum"]["assignments"] = json!([
        { "x": 1, "y": 0, "voice": 0 }, { "x": 1, "y": 0, "voice": 3 }
    ]);
    assert!(validate_config_payload(&repeated)
        .unwrap_err()
        .contains("duplicates a drum cell"));
}

#[test]
fn drum_sound_selector_resets_only_selected_voice_and_preserves_cells_and_inactive_settings() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    payload["runtimeConfig"]["instruments"][0]["noteBehavior"] = json!("hold");
    payload["runtimeConfig"]["instruments"][0]["drum"]["voices"][3]["tonePct"] = json!(23);
    payload["runtimeConfig"]["instruments"][0]["drum"]["voices"][3]["tuneSemis"] = json!(4);
    payload["runtimeConfig"]["instruments"][0]["drum"]["assignments"] =
        json!([{ "x": 1, "y": 0, "voice": 3, "tuneSemis": 13 }]);
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(
        runner.note_behaviors[0],
        platform_core::NoteBehavior::Oneshot
    );
    assert_eq!(
        runner.instrument_audio_config(0).unwrap()["noteBehavior"],
        "oneshot"
    );
    let other = runner.instruments[0].drum_config["voices"][2].clone();
    assert!(runner.menu.focus_item_key("instruments.0.drum.voice"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "main", "delta": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.drum_selected_voices[0], 3);
    assert!(runner
        .menu
        .focus_item_key("instruments.0.drum.voices.3.sound"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    let kit = runner.instruments[0].drum_config.clone();
    assert_eq!(
        kit["voices"][3],
        super::super::drum_config::drum_voice_default("low_tom").unwrap()
    );
    assert_eq!(kit["voices"][2], other);
    assert_eq!(
        kit["assignments"],
        json!([{ "x": 1, "y": 0, "voice": 3, "tuneSemis": 13 }])
    );
    let saved = runner.config_payload();
    assert_eq!(
        RuntimeConfigDto::from_value(&saved["runtimeConfig"])
            .unwrap()
            .to_value()
            .unwrap()["instruments"][0]["drum"],
        kit
    );
    let mut synth = saved.clone();
    synth["runtimeConfig"]["instruments"][0]["type"] = json!("synth");
    runner.apply_config_payload(synth).unwrap();
    runner.apply_config_payload(saved).unwrap();
    assert_eq!(
        runner.instruments[0].drum_config["assignments"],
        kit["assignments"]
    );
}
