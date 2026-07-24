use super::*;

fn numeric_binding(key: &str, min: f64, max: f64) -> NativeParamBinding {
    NativeParamBinding {
        key: key.into(),
        label: Some(key.into()),
        kind: "number".into(),
        min: Some(min),
        max: Some(max),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    }
}

#[test]
pub(crate) fn config_payload_includes_complete_sample_and_fx_param_shapes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();

    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["sample"]["baseVelocity"],
        100
    );
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["midiEngine"]["velocity"],
        100
    );
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["midiEngine"]["channel"],
        1
    );
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["ampEnv"].is_object());
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["filter"].is_object());
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["filterEnv"].is_object());
    assert!(payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"].is_object());
    assert!(payload["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"].is_object());

    payload["runtimeConfig"]["instruments"][0]["sample"]["baseVelocity"] = json!(72);
    payload["runtimeConfig"]["instruments"][0]["sample"]["ampEnv"] = json!({ "attackMs": 11 });
    payload["runtimeConfig"]["instruments"][0]["sample"]["filter"] =
        json!({ "type": "highpass", "cutoffHz": 1200 });
    payload["runtimeConfig"]["instruments"][0]["sample"]["filterEnv"] = json!({ "releaseMs": 222 });
    payload["runtimeConfig"]["instruments"][0]["midiEngine"] =
        json!({ "channel": 7, "velocity": 66, "durationMs": 444 });
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] =
        json!({ "type": "delay", "params": { "timeMs": 333, "feedback": 0.42, "mixPct": 44 } });
    payload["runtimeConfig"]["mixer"]["master"]["slots"][0] =
        json!({ "type": "distortion", "params": { "drive": 3.5, "clip": 0.75, "mixPct": 88 } });
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.instruments[0].sample_base_velocity, 72);
    assert_eq!(runner.instruments[0].sample_amp_env["attackMs"], 11);
    assert_eq!(runner.instruments[0].sample_filter["type"], "highpass");
    assert_eq!(runner.instruments[0].sample_filter_env["releaseMs"], 222);
    assert_eq!(runner.instruments[0].midi_velocity, 66);
    assert_eq!(runner.instruments[0].midi_channel, 7);
    assert_eq!(runner.instruments[0].midi_duration_ms, 444);
    assert_eq!(runner.fx_buses[0].slot1_params["feedback"], 0.42);
    assert_eq!(runner.global_fx_params[0]["drive"], 3.5);
    let round_trip = runner.config_payload();
    assert_eq!(
        round_trip["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["timeMs"],
        333
    );
    assert_eq!(
        round_trip["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]["clip"],
        0.75
    );
}

#[test]
pub(crate) fn interactive_exclusive_claim_conflict_is_transactional() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let exclusive = numeric_binding("mixer.buses.0.slot1.params.timeMs", 0.0, 2000.0);
    runner.set_param_binding_target("param:0:x:0", Some(exclusive.clone()));
    assert!(runner.param_mods[0].x[0].is_some());
    assert!(runner.menu.focus_item_key("param:0:y:0"));
    let before_payload = runner.config_payload();
    let before_revision = runner.config_revision;
    let before_dirty = runner.dirty_revision;
    let before_autosave = runner.pending.pending_autosave_payload_due_at;
    let before_focus = runner.menu.current_focus_path();

    runner.set_param_binding_target("param:0:y:0", Some(exclusive));

    assert!(runner.param_mods[0].y[0].is_none());
    assert_eq!(runner.config_payload(), before_payload);
    assert_eq!(runner.config_revision, before_revision);
    assert_eq!(runner.dirty_revision, before_dirty);
    assert_eq!(
        runner.pending.pending_autosave_payload_due_at,
        before_autosave
    );
    assert_eq!(runner.menu.current_focus_path(), before_focus);
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Mapping rejected: target already claimed")
    );
}

#[test]
pub(crate) fn physical_global_lfo_assignment_refreshes_labels_and_survives_reload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let action_key = "linkLfos.0.target.instruments.0.mixer.volume";
    assert!(runner.menu.focus_item_key(action_key));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner.link_lfos[0]
            .target
            .as_ref()
            .map(|binding| binding.key.as_str()),
        Some("instruments.0.mixer.volume")
    );
    assert!(runner
        .menu
        .current_label()
        .is_some_and(|label| label.contains("Volume")));

    let payload = runner.config_payload();
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(
        restored.link_lfos[0]
            .target
            .as_ref()
            .map(|binding| binding.key.as_str()),
        Some("instruments.0.mixer.volume")
    );
}

#[test]
pub(crate) fn physical_layer_and_play_xy_assignments_refresh_and_round_trip() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for action_key in [
        "param:0:x:0.instruments.0.mixer.volume",
        "xy:x.instruments.0.mixer.panPos",
    ] {
        assert!(runner.menu.focus_item_key(action_key));
        runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();
    }

    assert_eq!(
        runner.param_mods[0].x[0]
            .as_ref()
            .map(|binding| binding.key.as_str()),
        Some("instruments.0.mixer.volume")
    );
    assert_eq!(
        runner
            .xy_x_binding
            .as_ref()
            .map(|binding| binding.key.as_str()),
        Some("instruments.0.mixer.panPos")
    );
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored
        .apply_config_payload(runner.config_payload())
        .unwrap();
    assert_eq!(
        restored.param_mods[0].x[0]
            .as_ref()
            .map(|binding| binding.key.as_str()),
        Some("instruments.0.mixer.volume")
    );
    assert_eq!(
        restored
            .xy_x_binding
            .as_ref()
            .map(|binding| binding.key.as_str()),
        Some("instruments.0.mixer.panPos")
    );
}

#[test]
pub(crate) fn physical_link_entry_selects_active_layer_by_stable_label() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_layer_index = 1;
    let link_index = runner
        .menu
        .root
        .children
        .iter()
        .position(|item| item.label == "Link")
        .unwrap();
    runner.menu.state.cursor = link_index;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.menu.current_label(), Some("L2: life"));
}
