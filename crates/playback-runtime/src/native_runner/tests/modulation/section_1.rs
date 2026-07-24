use super::*;

#[test]
pub(crate) fn pulses_value_lanes_load_into_runner_and_menu_curve_edits_apply() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["layers"][0]["pulses"]["x"]["velocity"] = json!({
        "enabled": true,
        "from": 12,
        "to": 99,
        "gridOffset": 2,
        "curve": "curve"
    });
    payload["runtimeConfig"]["layers"][0]["pulses"]["y"]["filterResonance"] = json!({
        "enabled": true,
        "from": 3,
        "to": 77,
        "gridOffset": -1,
        "curve": "linear"
    });

    runner.apply_config_payload(payload).unwrap();

    assert!(runner.pulses_layers[0].x_velocity.enabled);
    assert_eq!(runner.pulses_layers[0].x_velocity.from, 12);
    assert_eq!(runner.pulses_layers[0].x_velocity.to, 99);
    assert_eq!(runner.pulses_layers[0].x_velocity.grid_offset, 2);
    assert_eq!(runner.pulses_layers[0].x_velocity.curve, "curve");
    assert!(runner.pulses_layers[0].y_filter_resonance.enabled);

    runner.menu.rebuild(runner.menu_config());
    runner.menu.turn_key("layers.0.pulses.x.velocity.curve", -1);
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.pulses_layers[0].x_velocity.curve, "linear");
}

#[test]
pub(crate) fn canonical_global_modulation_has_eight_slots_and_no_layer_ownership() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let runtime = runner.config_payload()["runtimeConfig"].clone();
    assert_eq!(runtime["linkLfos"].as_array().unwrap().len(), 8);
    assert!(runtime["xy"]["x"].is_null());
    assert!(runtime["xy"]["y"].is_null());
    for layer in runtime["layers"].as_array().unwrap() {
        assert!(layer.get("linkLfo").is_none());
        assert!(layer.get("xy").is_none());
    }
}

#[test]
pub(crate) fn legacy_lfos_xy_and_aux_banks_migrate_transactionally() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut legacy = legacy_payload(runner.config_payload());
    legacy["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("linkLfos");
    legacy["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("xy");
    legacy["runtimeConfig"]["layers"][2]["linkLfo"] = json!({
        "enabled": true,
        "target": { "key": "instruments.0.mixer.volume", "kind": "number", "min": 0, "max": 100, "step": 1 },
        "period": "1/4",
        "depthPct": 40
    });
    legacy["runtimeConfig"]["layers"][0]["xy"] = json!({
        "x": { "key": "instruments.0.mixer.panPos", "kind": "number", "min": 0, "max": 32, "step": 1 },
        "y": null,
        "xInvert": true,
        "yInvert": false
    });
    legacy["runtimeConfig"]["activeLayerIndex"] = json!(0);
    legacy["runtimeConfig"]["auxBindings"]["aux1"] = json!({
        "turnKey": "layers.2.linkLfo.depthPct"
    });
    legacy["runtimeConfig"]["shiftAuxBindings"]["aux2"] = json!({
        "turnKey": "layers.2.linkLfo.period"
    });

    runner.apply_config_payload(legacy).unwrap();
    let runtime = runner.config_payload()["runtimeConfig"].clone();
    assert_eq!(runtime["linkLfos"][2]["depthPct"], 40);
    assert_eq!(runtime["xy"]["x"]["key"], "instruments.0.mixer.panPos");
    assert_eq!(
        runtime["auxBindings"]["aux1"]["turnKey"],
        "linkLfos.2.depthPct"
    );
    assert_eq!(
        runtime["shiftAuxBindings"]["aux2"]["turnKey"],
        "linkLfos.2.period"
    );
    assert!(runner
        .display
        .toast
        .as_ref()
        .is_some_and(|toast| toast.message.contains("Migrated legacy modulation")));
}

#[test]
pub(crate) fn legacy_global_lfo_bank_wins_per_layer_conflicts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut legacy = legacy_payload(runner.config_payload());
    legacy["runtimeConfig"]["layers"][0]["linkLfo"] = json!({"depthPct": 99});
    legacy["runtimeConfig"]["linkLfos"][0]["depthPct"] = json!(11);

    runner.apply_config_payload(legacy).unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["linkLfos"][0]["depthPct"],
        11
    );
}

#[test]
pub(crate) fn canonical_v2_rejects_legacy_fields_without_mutating_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let before = runner.config_payload();
    let mut payload = before.clone();
    payload["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": false,
        "target": null,
        "period": "1/1",
        "depthPct": 100
    });

    assert!(runner.apply_config_payload(payload).is_err());
    assert_eq!(runner.config_payload(), before);
}

#[test]
pub(crate) fn canonical_v2_rejects_legacy_xy_aux_keys_and_noncanonical_lfo_shape() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let before = runner.config_payload();
    let mutations: [fn(&mut Value); 4] = [
        |payload: &mut Value| {
            payload["runtimeConfig"]["layers"][0]["xy"] = json!({"x": null, "y": null});
        },
        |payload: &mut Value| {
            payload["runtimeConfig"]["auxBindings"]["aux1"] =
                json!({"turnKey": "layers.0.linkLfo.depthPct"});
        },
        |payload: &mut Value| {
            payload["runtimeConfig"]["linkLfos"] = json!([]);
        },
        |payload: &mut Value| {
            payload["runtimeConfig"]["linkLfos"][0]["phasePulses"] = json!(2);
        },
    ];
    for (index, mutate) in mutations.into_iter().enumerate() {
        let mut payload = before.clone();
        mutate(&mut payload);
        assert!(
            runner.apply_config_payload(payload).is_err(),
            "canonical mutation {index} was accepted"
        );
        assert_eq!(runner.config_payload(), before);
    }
}

#[test]
pub(crate) fn legacy_exclusive_claims_keep_stable_first_and_report() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut legacy = legacy_payload(runner.config_payload());
    let binding = json!({
        "key": "layers.0.pulses.scanSections",
        "kind": "number",
        "min": 1,
        "max": 8,
        "step": 1
    });
    legacy["runtimeConfig"]["layers"][0]["paramMods"]["x"][0] = binding.clone();
    legacy["runtimeConfig"]["layers"][0]["paramMods"]["y"][0] = binding;

    runner.apply_config_payload(legacy).unwrap();

    let mods = &runner.config_payload()["runtimeConfig"]["layers"][0]["paramMods"];
    assert!(mods["x"][0].is_object());
    assert!(mods["y"][0].is_null());
    assert!(runner
        .display
        .toast
        .as_ref()
        .is_some_and(|toast| toast.message.contains("duplicate exclusive")));
}

#[test]
pub(crate) fn legacy_xy_uses_first_present_layer_when_active_layer_has_none() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut legacy = legacy_payload(runner.config_payload());
    legacy["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("linkLfos");
    legacy["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("xy");
    legacy["runtimeConfig"]["activeLayerIndex"] = json!(0);
    legacy["runtimeConfig"]["layers"][1]["xy"] = json!({
        "x": { "key": "instruments.0.mixer.volume", "kind": "number", "min": 0, "max": 100, "step": 1 },
        "y": null,
        "xInvert": false,
        "yInvert": true
    });

    runner.apply_config_payload(legacy).unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["xy"]["x"]["key"],
        "instruments.0.mixer.volume"
    );
}

#[test]
pub(crate) fn canonical_duplicate_exclusive_claim_is_rejected() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    let binding = json!({
        "key": "layers.0.pulses.scanSections",
        "kind": "number",
        "min": 1,
        "max": 8,
        "step": 1
    });
    payload["runtimeConfig"]["layers"][0]["paramMods"]["x"][0] = binding.clone();
    payload["runtimeConfig"]["layers"][0]["paramMods"]["y"][0] = binding;

    assert!(runner.apply_config_payload(payload).is_err());
}

#[test]
pub(crate) fn global_lfo_menu_uses_keyed_slots_and_target_only_fast_paths() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.link_lfos[0].target = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.menu.rebuild(runner.menu_config());
    for index in 0..8 {
        assert!(runner
            .menu
            .focus_item_key(&format!("linkLfos.{index}.depthPct")));
    }
    assert!(runner.menu.focus_item_key("linkLfos.0.depthPct"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -25, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.link_lfos[0].depth_pct, 75);
    assert!(runner.menu.focus_item_key("linkLfos.0.period"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.link_lfos[0].period, "1/1T");
    assert!(runner.menu.focus_item_key("linkLfos.0.target.rangeMin"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 7, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner.link_lfos[0].target.as_ref().unwrap().user_min,
        Some(7.0)
    );
}

#[test]
pub(crate) fn lfo_assignment_rejects_exclusive_and_lfo_config_targets() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for key in ["sound.noteLengthMs", "linkLfos.1.depthPct"] {
        runner.set_param_binding_target(
            "linkLfos.0.target",
            Some(NativeParamBinding {
                key: key.into(),
                label: None,
                kind: "number".into(),
                min: Some(0.0),
                max: Some(100.0),
                step: Some(1.0),
                user_min: None,
                user_max: None,
                options: vec![],
                invert: false,
            }),
        );
        assert!(runner.link_lfos[0].target.is_none());
    }
}

#[test]
pub(crate) fn global_lfo_phase_is_transient_and_old_audio_path_is_not_run() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.link_lfos[0].phase_pulses = 17;
    let payload = runner.config_payload();
    assert!(payload["runtimeConfig"]["linkLfos"][0]
        .get("phasePulses")
        .is_none());
    runner.transport.transport = RuntimeTransportState::Playing;
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands } if commands.iter().any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer { .. }
                | RuntimeAudioCommand::SetFxBusMixer { .. }
                | RuntimeAudioCommand::SetFxBusSlot { .. }
                | RuntimeAudioCommand::SetGlobalFxSlot { .. }
        ))
    )));
}
