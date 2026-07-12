use super::*;

#[test]
pub(crate) fn auto_map_context_updates_after_navigation_only_group_enter() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let worlds_items = &runner.menu.root.children[0].children[0].children;
    let interval_cursor = child_index_by_key(
        worlds_items,
        "layers.0.worlds.behaviorConfig.randomTickInterval",
    );

    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = interval_cursor;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    let slot = runner.effective_aux_slot(0);
    assert_eq!(
        slot.turn.as_ref().map(|turn| turn.key.as_str()),
        Some("algorithmStep")
    );
    assert_eq!(
        slot.turn.as_ref().map(|turn| turn.label.as_str()),
        Some("Step")
    );
}

#[test]
pub(crate) fn auto_map_behavior_context_uses_worlds_label() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let worlds_items = &runner.menu.root.children[0].children[0].children;
    let interval_cursor = child_index_by_key(
        worlds_items,
        "layers.0.worlds.behaviorConfig.randomTickInterval",
    );
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = interval_cursor;
    runner.ui.fn_held = true;
    runner.fn_hold_started_at = Some(Instant::now() - Duration::from_millis(1600));

    let snapshot = runner.snapshot().unwrap();
    let lines = snapshot["display"]["lines"].as_array().unwrap();

    assert_eq!(snapshot["display"]["title"], "AUTO MAP");
    assert_eq!(lines[0], "Build");
}

#[test]
pub(crate) fn auto_map_env_and_osc_pages_drive_expected_slots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let (synth_group, amp_env_group, osc1_group) = {
        let instrument_children = &runner.menu.root.children[2].children[0].children[0].children;
        let synth_group = child_index_by_label(instrument_children, "Synth");
        let synth_children = &instrument_children[synth_group].children;
        let amp_env_group = child_index_by_label(synth_children, "Amp Env");
        let osc1_group = child_index_by_label(synth_children, "Osc 1");
        (synth_group, amp_env_group, osc1_group)
    };
    runner.menu.state.stack = vec![2, 0, 0, synth_group, amp_env_group];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux3", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner
            .menu
            .number_for_key("instruments.0.synth.ampEnv.sustainPct"),
        Some(71)
    );

    runner.menu.state.stack = vec![2, 0, 0, synth_group, osc1_group];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux3", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner
            .menu
            .number_for_key("instruments.0.synth.osc1.detuneCents"),
        Some(1)
    );
}

#[test]
pub(crate) fn auto_map_fx_slot_uses_effect_specific_param_layout() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slot1_type = "vibrato".into();
    runner.fx_buses[0].slot1_params = json!({
        "rateHz": 0.8,
        "depthMs": 6,
        "baseMs": 8,
        "mixPct": 100
    });
    runner.menu.rebuild(runner.menu_config());

    let voice_children = &runner.menu.root.children[2].children;
    let fx_buses_group = child_index_by_label(voice_children, "FX Buses");
    let bus_group = 0;
    let slot1_group = 0;
    let slot1_children =
        &voice_children[fx_buses_group].children[bus_group].children[slot1_group].children;
    let rate_cursor = child_index_by_key(slot1_children, "mixer.buses.0.slot1.params.rateHz");
    runner.menu.state.stack = vec![2, fx_buses_group, bus_group, slot1_group];
    runner.menu.state.cursor = rate_cursor;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux3", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner
            .menu
            .number_for_key("mixer.buses.0.slot1.params.baseMs"),
        Some(81)
    );
}

#[test]
pub(crate) fn auto_map_global_fx_covers_vinyl_params() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.global_fx_slots[0] = "vinyl".into();
    runner.global_fx_params[0] = json!({
        "saturationPct": 15,
        "cracklePct": 8,
        "warpDepthPct": 5,
        "mixPct": 100
    });
    runner.menu.rebuild(runner.menu_config());

    let voice_children = &runner.menu.root.children[2].children;
    let global_fx_group = child_index_by_label(voice_children, "Global FX");
    let slot_group = 0;
    let slot_children = &voice_children[global_fx_group].children[slot_group].children;
    let mix_cursor = child_index_by_key(slot_children, "mixer.master.slots.0.params.mixPct");
    runner.menu.state.stack = vec![2, global_fx_group, slot_group];
    runner.menu.state.cursor = mix_cursor;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux2", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner
            .menu
            .number_for_key("mixer.master.slots.0.params.cracklePct"),
        Some(9)
    );
}

#[test]
pub(crate) fn fn_held_shows_auto_aux_mapping_overlay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = synth_stack(&runner, "Filter");
    runner.menu.state.cursor = 1;
    runner.ui.fn_held = true;
    runner.fn_hold_started_at = Some(Instant::now() - Duration::from_millis(1600));

    let snapshot = runner.snapshot().unwrap();
    let lines = snapshot["display"]["lines"].as_array().unwrap();

    assert_eq!(snapshot["display"]["title"], "AUTO MAP");
    assert_eq!(lines[0], "Synth Filter");
    assert_eq!(lines[1], "A1 Cutoff");
    assert_eq!(lines[2], "A2 Res");
    assert_eq!(lines[3], "A3 Env");
    assert_eq!(lines.len(), 4);
    assert_eq!(runner.menu.state.stack, synth_stack(&runner, "Filter"));
    assert_eq!(runner.menu.state.cursor, 1);
}

#[test]
pub(crate) fn fn_held_shows_custom_aux_mapping_overlay_when_no_auto_map_applies() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.aux_auto_map_enabled = false;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: Some(NativeMenuAction::ResetBehavior),
    });
    runner.ui.fn_held = true;
    runner.fn_hold_started_at = Some(Instant::now() - Duration::from_millis(1600));

    let snapshot = runner.snapshot().unwrap();
    let lines = snapshot["display"]["lines"].as_array().unwrap();

    assert_eq!(snapshot["display"]["title"], "CUSTOM MAP");
    assert_eq!(lines[1], "A1 Master Vol/!Reset");
    assert_eq!(lines[2], "A2 -");
}

#[test]
pub(crate) fn aux_overlay_waits_for_fn_hold_delay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = synth_stack(&runner, "Filter");
    runner.menu.state.cursor = 1;
    runner.ui.fn_held = true;
    runner.fn_hold_started_at = Some(Instant::now());

    let snapshot = runner.snapshot().unwrap();

    assert_ne!(snapshot["display"]["title"], "AUTO MAP");
}

#[test]
pub(crate) fn fn_aux_bind_sets_explicit_toast_and_marks_config_dirty() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 2];
    runner.menu.state.cursor = 0;
    runner.ui.fn_held = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner.toast.as_ref().unwrap().message,
        "Click-1: Bound turn: Master Vol"
    );
    assert!(runner.config_dirty);
}

#[test]
pub(crate) fn aux_click_action_toast_is_shown_for_platform_effect_actions() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: None,
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner.toast.as_ref().unwrap().message,
        "Click-1: MIDI Panic"
    );
}
