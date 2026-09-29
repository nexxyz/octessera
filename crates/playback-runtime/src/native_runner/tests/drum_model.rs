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
    for (index, (sound, decay, tone, sweep_semis, sweep_ms, noise_mix_pct)) in [
        ("kick", 420, 35, 24, 55, 12),
        ("snare", 220, 70, 3, 25, 80),
        ("closed_hat", 85, 90, 0, 0, 95),
        ("open_hat", 650, 85, 0, 0, 95),
        ("low_tom", 500, 50, 8, 70, 15),
        ("high_tom", 320, 60, 6, 50, 15),
        ("clap", 240, 80, 0, 0, 95),
        ("rim", 95, 80, 0, 0, 35),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            drum["voices"][index],
            json!({
                "sound": sound, "tuneSemis": 0, "decayMs": decay, "tonePct": tone, "attackMs": 0,
                "sweepSemis": sweep_semis,
                "sweepMs": sweep_ms,
                "noiseMixPct": noise_mix_pct,
                "levelPct": 100,
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
        ("sweepSemis", json!(37)),
        ("sweepMs", json!(201)),
        ("noiseMixPct", json!(101)),
        ("levelPct", json!(101)),
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

fn remove_new_drum_fields(payload: &mut Value) {
    for instrument in payload["runtimeConfig"]["instruments"]
        .as_array_mut()
        .unwrap()
    {
        if let Some(voices) = instrument
            .get_mut("drum")
            .and_then(|drum| drum.get_mut("voices"))
            .and_then(Value::as_array_mut)
        {
            for voice in voices {
                for field in ["sweepSemis", "sweepMs", "noiseMixPct", "levelPct"] {
                    voice.as_object_mut().unwrap().remove(field);
                }
            }
        }
    }
}

#[test]
fn old_complete_drum_config_and_patch_fill_neutral_values_by_saved_sound() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut current = runner.config_payload();
    current["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    for (index, value) in [7, 8, 9, 10, 11, 12, 13, 14].into_iter().enumerate() {
        current["runtimeConfig"]["instruments"][0]["drum"]["voices"][index]["sweepSemis"] =
            json!(value);
    }
    runner.apply_config_payload(current.clone()).unwrap();

    let mut old_full = current.clone();
    let voices = old_full["runtimeConfig"]["instruments"][0]["drum"]["voices"]
        .as_array_mut()
        .unwrap();
    voices.swap(0, 1);
    remove_new_drum_fields(&mut old_full);
    runner.apply_config_payload(old_full.clone()).unwrap();
    for voice in runner.instruments[0].drum_config["voices"]
        .as_array()
        .unwrap()
    {
        let sound = voice["sound"].as_str().unwrap();
        let defaults = super::super::drum_config::drum_voice_default(sound).unwrap();
        for field in ["sweepSemis", "sweepMs", "noiseMixPct", "levelPct"] {
            assert_eq!(voice[field], defaults[field]);
        }
    }

    let mut current = runner.config_payload();
    for (index, value) in [20, 21, 22, 23, 24, 25, 26, 27].into_iter().enumerate() {
        current["runtimeConfig"]["instruments"][0]["drum"]["voices"][index]["sweepMs"] =
            json!(value);
    }
    runner.apply_config_payload(current.clone()).unwrap();
    let mut old_patch = portable_patch_projection(&current).unwrap();
    old_patch["runtimeConfig"]["instruments"][0]["drum"]["voices"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    remove_new_drum_fields(&mut old_patch);
    let old_current = {
        let mut payload = current;
        remove_new_drum_fields(&mut payload);
        payload
    };
    let prepared = prepare_patch_payload(old_patch.clone(), &old_current).unwrap();
    for voice in prepared.payload["runtimeConfig"]["instruments"][0]["drum"]["voices"]
        .as_array()
        .unwrap()
    {
        let sound = voice["sound"].as_str().unwrap();
        let defaults = super::super::drum_config::drum_voice_default(sound).unwrap();
        for field in ["sweepSemis", "sweepMs", "noiseMixPct", "levelPct"] {
            assert_eq!(voice[field], defaults[field]);
        }
    }
    runner
        .apply_patch_payload_preserving_device(old_patch)
        .unwrap();
    for voice in runner.instruments[0].drum_config["voices"]
        .as_array()
        .unwrap()
    {
        let sound = voice["sound"].as_str().unwrap();
        let defaults = super::super::drum_config::drum_voice_default(sound).unwrap();
        for field in ["sweepSemis", "sweepMs", "noiseMixPct", "levelPct"] {
            assert_eq!(voice[field], defaults[field]);
        }
    }
}

#[test]
fn partial_drum_edit_inherits_new_values_and_invalid_explicit_values_do_not_commit() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    payload["runtimeConfig"]["instruments"][0]["drum"]["voices"][0]["sweepSemis"] = json!(19);
    payload["runtimeConfig"]["instruments"][0]["drum"]["voices"][0]["sweepMs"] = json!(87);
    payload["runtimeConfig"]["instruments"][0]["drum"]["voices"][0]["noiseMixPct"] = json!(61);
    payload["runtimeConfig"]["instruments"][0]["drum"]["voices"][0]["levelPct"] = json!(73);
    runner.apply_config_payload(payload).unwrap();
    runner
        .apply_patch_payload_preserving_device(json!({
            "kind": "octessera.patch",
            "schemaVersion": 2,
            "runtimeConfig": { "instruments": [{ "drum": { "voices": [{ "tonePct": 44 }] } }] }
        }))
        .unwrap();
    assert_eq!(
        runner.instruments[0].drum_config["voices"][0]["sweepSemis"],
        19
    );
    let saved = runner.config_payload();
    let decoded = RuntimeConfigDto::from_value(&saved["runtimeConfig"])
        .unwrap()
        .to_value()
        .unwrap();
    assert_eq!(
        decoded["instruments"][0]["drum"],
        runner.instruments[0].drum_config
    );
    let mut reloaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    reloaded.apply_config_payload(saved).unwrap();
    assert_eq!(
        reloaded.instruments[0].drum_config,
        runner.instruments[0].drum_config
    );

    for (field, bad) in [
        ("sweepSemis", json!(37)),
        ("sweepMs", json!(201)),
        ("noiseMixPct", json!(101)),
        ("levelPct", json!(101)),
    ] {
        let before = runner.config_payload();
        let mut invalid = before.clone();
        invalid["runtimeConfig"]["instruments"][0]["drum"]["voices"][0][field] = bad;
        assert!(runner.apply_config_payload(invalid).is_err(), "{field}");
        assert_eq!(runner.config_payload(), before, "{field}");
    }
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
