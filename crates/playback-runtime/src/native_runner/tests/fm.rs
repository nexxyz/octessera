use super::*;

#[test]
fn fm_defaults_validate_and_legacy_config_loads_without_a_schema_bump() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let defaults = runner.instruments[0].fm_config.clone();
    assert_eq!(defaults["ratio"], "2");
    assert_eq!(defaults["index"], 50);
    assert_eq!(
        defaults["indexEnv"],
        json!({
            "attackMs": 0, "decayMs": 250, "sustainPct": 20, "releaseMs": 120
        })
    );
    assert_eq!(
        defaults["amp"],
        json!({ "gainPct": 80, "velocitySensitivityPct": 100 })
    );
    assert_eq!(
        defaults["ampEnv"],
        json!({
            "attackMs": 5, "decayMs": 300, "sustainPct": 70, "releaseMs": 350
        })
    );
    assert_eq!(
        defaults["filter"],
        runner.instruments[0].synth_config["filter"]
    );
    assert_eq!(
        defaults["filterEnv"],
        runner.instruments[0].synth_config["filterEnv"]
    );

    let mut legacy = runner.config_payload();
    legacy["runtimeConfig"]["instruments"][0]
        .as_object_mut()
        .unwrap()
        .remove("fm");
    legacy["runtimeConfig"]["instruments"][0]
        .as_object_mut()
        .unwrap()
        .remove("name");
    legacy["runtimeConfig"]["instruments"][0]["type"] = json!("fm");
    runner.apply_config_payload(legacy).unwrap();
    assert_eq!(runner.config_payload()["schemaVersion"], 2);
    assert_eq!(runner.instruments[0].kind, "fm");
    assert_eq!(runner.instruments[0].name, "FM");
    assert_eq!(runner.instruments[0].fm_config, defaults);
}

#[test]
fn fm_full_legacy_load_resets_missing_fm_but_partial_patch_and_slot_preserve_it() {
    for unversioned in [false, true] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let defaults = runner.instruments[0].fm_config.clone();
        let mut legacy = runner.config_payload();
        for slot in legacy["runtimeConfig"]["instruments"]
            .as_array_mut()
            .unwrap()
        {
            slot.as_object_mut().unwrap().remove("fm");
        }
        legacy["runtimeConfig"]["instruments"][0]["type"] = json!("fm");
        legacy["runtimeConfig"]["instruments"][0]["synth"]["amp"]["gainPct"] = json!(62);
        legacy["runtimeConfig"]["instruments"][0]["sample"]["amp"]["gainPct"] = json!(91);

        let mut edited = runner.config_payload();
        edited["runtimeConfig"]["instruments"][0]["fm"]["ratio"] = json!("8");
        edited["runtimeConfig"]["instruments"][0]["fm"]["index"] = json!(87);
        edited["runtimeConfig"]["instruments"][1]["fm"]["index"] = json!(89);
        runner.apply_config_payload(edited).unwrap();
        runner
            .apply_patch_payload_preserving_device(json!({
                "runtimeConfig": { "instruments": [{ "type": "fm" }] }
            }))
            .unwrap();
        assert_eq!(runner.instruments[0].fm_config["index"], 87);
        super::super::apply_payload_instrument_values::apply_instrument_fm_payload(
            &json!({ "type": "synth" }),
            &mut runner.instruments[0],
        );
        assert_eq!(runner.instruments[0].fm_config["index"], 87);

        runner
            .apply_config_payload(if unversioned {
                unversioned_payload(legacy)
            } else {
                legacy
            })
            .unwrap();
        assert_eq!(runner.instruments[0].fm_config, defaults);
        assert_eq!(runner.instruments[1].fm_config, defaults);
        assert_eq!(runner.instruments[0].synth_gain_pct, 62);
        assert_eq!(runner.instruments[0].sample_gain_pct, 91);
    }
}

#[test]
fn fm_schema_bounds_and_selector_reject_invalid_saved_values() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let initial = runner.config_payload();
    for (path, bad) in [
        ("ratio", json!(2)),
        ("ratio", json!("7")),
        ("index", json!(-1)),
        ("index", json!(101)),
    ] {
        let mut payload = initial.clone();
        payload["runtimeConfig"]["instruments"][0]["fm"][path] = bad;
        assert!(validate_config_payload(&payload).is_err(), "{path}");
    }
    for (group, field, bad) in [
        ("indexEnv", "releaseMs", json!(10001)),
        ("indexEnv", "sustainPct", json!(101)),
        ("amp", "gainPct", json!(101)),
        ("ampEnv", "attackMs", json!(5001)),
        ("filter", "cutoffHz", json!(19)),
        ("filter", "type", json!("broken")),
        ("filterEnv", "decayMs", json!(5001)),
    ] {
        let mut payload = initial.clone();
        payload["runtimeConfig"]["instruments"][0]["fm"][group][field] = bad;
        assert!(
            validate_config_payload(&payload).is_err(),
            "{group}.{field}"
        );
    }
    let mut payload = initial;
    payload["runtimeConfig"]["instruments"][0]["fm"]["ratio"] = json!("0.5");
    payload["runtimeConfig"]["instruments"][0]["fm"]["index"] = json!(0);
    validate_config_payload(&payload).unwrap();
    payload["runtimeConfig"]["instruments"][0]["fm"]["index"] = json!(100);
    validate_config_payload(&payload).unwrap();
}

#[test]
fn fm_saved_state_survives_type_switch_round_trip_clone_reset_and_audio_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("fm");
    payload["runtimeConfig"]["instruments"][0]["fm"]["ratio"] = json!("8");
    payload["runtimeConfig"]["instruments"][0]["fm"]["index"] = json!(74);
    payload["runtimeConfig"]["instruments"][0]["mixer"]["route"] = json!("fx_bus_2");
    runner.apply_config_payload(payload).unwrap();
    let fm = runner.instruments[0].fm_config.clone();
    assert_eq!(runner.instrument_audio_config(0).unwrap()["fm"], fm);
    assert_eq!(
        runner.instrument_audio_config(0).unwrap()["mixer"]["route"],
        "fx_bus_2"
    );
    let saved = runner.config_payload();
    assert_eq!(saved["runtimeConfig"]["instruments"][0]["fm"], fm);
    let dto = RuntimeConfigDto::from_value(&saved["runtimeConfig"]).unwrap();
    assert_eq!(dto.to_value().unwrap()["instruments"][0]["fm"], fm);

    let mut synth_payload = saved.clone();
    synth_payload["runtimeConfig"]["instruments"][0]["type"] = json!("synth");
    runner.apply_config_payload(synth_payload).unwrap();
    assert_eq!(runner.instruments[0].fm_config, fm);
    runner.apply_config_payload(saved).unwrap();
    assert_eq!(runner.instruments[0].kind, "fm");
    assert_eq!(runner.instruments[0].fm_config, fm);

    runner.instruments[1] = NativeInstrumentSlot::reset(1);
    runner
        .execute_menu_action(NativeMenuAction::CloneInstrument { index: 0 })
        .unwrap();
    confirm_current_dialog(&mut runner);
    assert_eq!(runner.instruments[1].kind, "fm");
    assert_eq!(runner.instruments[1].fm_config, fm);
    runner
        .execute_menu_action(NativeMenuAction::ResetInstrument { index: 1 })
        .unwrap();
    confirm_current_dialog(&mut runner);
    assert_eq!(
        runner.instruments[1].fm_config,
        super::super::synth_config::fm_default_config()
    );
}

#[test]
fn fm_scalar_outbox_coalesces_and_replacement_advances_generation() {
    let mut outbox = super::super::outbox::NativeRunnerOutbox::default();
    let param = |value| RuntimeAudioCommand::SetFmParam {
        instrument_slot: 0,
        generation: 0,
        path: "fm.index".into(),
        value,
    };
    outbox.push_audio_command(param(51.0));
    outbox.push_audio_command(param(52.0));
    let commands = outbox.drain_audio_commands();
    assert!(matches!(
        &commands[..],
        [RuntimeAudioCommand::SetFmParam {
            generation: 0,
            value: 52.0,
            ..
        }]
    ));
    outbox.push_audio_command(param(53.0));
    outbox.push_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0,
        generation: 0,
        config: json!({ "type": "fm" }),
    });
    assert!(matches!(
        &outbox.drain_audio_commands()[..],
        [RuntimeAudioCommand::SetInstrumentSlot { generation: 1, .. }]
    ));
    outbox.push_audio_command(param(54.0));
    assert!(matches!(
        &outbox.drain_audio_commands()[..],
        [RuntimeAudioCommand::SetFmParam { generation: 1, .. }]
    ));
    let encoded = serde_json::to_value(param(55.0)).unwrap();
    assert_eq!(
        encoded,
        json!({
            "type": "set_fm_param", "instrumentSlot": 0, "generation": 0,
            "path": "fm.index", "value": 55.0
        })
    );
    assert_eq!(
        serde_json::from_value::<RuntimeAudioCommand>(encoded).unwrap(),
        param(55.0)
    );
}

#[test]
fn fm_held_notes_transpose_and_route_to_internal_audio_on_the_original_slot() {
    use crate::native_runner::modulation_sampler::apply_sampler_assignments_for_instruments_routed;
    use std::collections::BTreeMap;

    let mut instrument = NativeInstrumentSlot::new(0);
    instrument.kind = "fm".into();
    instrument.route = "fx_bus_2".into();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0] = instrument.clone();
    assert!(runner.play_transpose_target_eligible(0, "note_on"));
    assert!(runner.play_transpose_target_eligible(0, "note_off"));
    let intent = platform_core::CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let mut held = BTreeMap::new();
    let note_on = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 90,
            duration_ms: None,
        }],
        std::slice::from_ref(&intent),
        0,
        &[instrument.clone()],
        None,
        7,
        Some(&mut held),
    );
    assert!(note_on.midi.is_empty());
    assert!(matches!(
        &note_on.audio[..],
        [MusicalEvent::NoteOn {
            channel: 0,
            note: 67,
            velocity: 90,
            duration_ms: None,
        }]
    ));
    assert_eq!(
        instrument_audio_payload(&instrument)["mixer"]["route"],
        "fx_bus_2"
    );

    let note_off = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }],
        &[intent],
        0,
        &[instrument],
        None,
        0,
        Some(&mut held),
    );
    assert!(note_off.midi.is_empty());
    assert!(matches!(
        &note_off.audio[..],
        [MusicalEvent::NoteOff {
            channel: 0,
            note: 67
        }]
    ));
    assert!(held.is_empty());
}
