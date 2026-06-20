use super::*;
use std::time::{Duration, Instant};

fn child_index_by_label(items: &[crate::native_menu::NativeMenuItem], label: &str) -> usize {
    items
        .iter()
        .position(|item| item.label == label)
        .unwrap_or_else(|| panic!("missing label {label}"))
}

fn child_index_by_key(items: &[crate::native_menu::NativeMenuItem], key: &str) -> usize {
    items
        .iter()
        .position(|item| item.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("missing key {key}"))
}

#[test]
fn system_aux_auto_map_toggle_round_trips_in_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["auxAutoMapEnabled"] = json!(false);

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["auxAutoMapEnabled"],
        false
    );
    assert_eq!(
        runner.snapshot().unwrap()["settings"]["auxAutoMapEnabled"],
        false
    );
}

#[test]
fn auto_map_updates_synth_filter_and_prefixes_selected_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
    runner.menu.state.cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
        })
        .unwrap();
    let snapshot = snapshot_from(&messages);

    assert_eq!(
        runner
            .menu
            .number_for_key("instruments.0.synth.filter.cutoffHz"),
        Some(223)
    );
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("1-Cutoff")));
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("2-Res")));
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("3-Env")));
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("4-Key")));
}

#[test]
fn auto_map_press_enters_sample_assign_and_prefixes_assign_action() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 2;

    let opened = runner.messages_with_snapshot().unwrap();
    assert!(snapshot_from(&opened)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "1!Assign"));

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
        })
        .unwrap();

    assert_eq!(runner.sample_assign, Some((0, 0)));
}

#[test]
fn auto_map_is_disabled_in_l2_sense_and_unbound_toast_uses_short_format() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![1, 1, 0];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
        })
        .unwrap();

    assert_eq!(runner.toast.as_ref().unwrap().message, "T1: No binding");
}

#[test]
fn custom_aux_binding_still_works_when_auto_map_is_disabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.aux_auto_map_enabled = false;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
        })
        .unwrap();

    assert_eq!(runner.ui.master_volume, 72);
}

#[test]
fn custom_aux_binding_overrides_auto_map_when_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
    runner.menu.state.cursor = 0;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
        })
        .unwrap();

    assert_eq!(runner.ui.master_volume, 72);
    assert_eq!(
        runner
            .menu
            .number_for_key("instruments.0.synth.filter.cutoffHz"),
        Some(222)
    );
}

#[test]
fn custom_binding_is_used_on_l1_non_mapped_rows_even_when_auto_map_is_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 0;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
        })
        .unwrap();

    assert_eq!(runner.ui.master_volume, 72);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l1"]["stepRate"],
        "1/8"
    );
}

#[test]
fn auto_map_l1_life_turn_and_press_follow_behavior_context() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let life_items = &runner.menu.root.children[0].children[0].children;
    let interval_cursor =
        child_index_by_key(life_items, "parts.0.l1.behaviorConfig.randomTickInterval");
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = interval_cursor;

    let before = runner
        .engine
        .model()
        .unwrap()
        .cells
        .iter()
        .filter(|cell| **cell)
        .count();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
        })
        .unwrap();
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l1"]["stepRate"],
        "1/4"
    );

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux2", "delta": 1 }),
        })
        .unwrap();
    assert_eq!(runner.behavior_config["randomCellsPerTick"], 1);

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux3" }),
        })
        .unwrap();
    let after = runner
        .engine
        .model()
        .unwrap()
        .cells
        .iter()
        .filter(|cell| **cell)
        .count();

    assert!(after >= before);
    assert_eq!(runner.toast.as_ref().unwrap().message, "S3: Spawn");
}

#[test]
fn auto_map_env_and_osc_pages_drive_expected_slots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let (synth_group, amp_env_group, oscillator_group, osc1_group) = {
        let instrument_children = &runner.menu.root.children[2].children[0].children[0].children;
        let synth_group = child_index_by_label(instrument_children, "Synth");
        let synth_children = &instrument_children[synth_group].children;
        let amp_env_group = child_index_by_label(synth_children, "Amp Env");
        let oscillator_group = child_index_by_label(synth_children, "Oscillator");
        let osc1_group = child_index_by_label(&synth_children[oscillator_group].children, "Osc 1");
        (synth_group, amp_env_group, oscillator_group, osc1_group)
    };
    runner.menu.state.stack = vec![2, 0, 0, synth_group, amp_env_group];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux4", "delta": 1 }),
        })
        .unwrap();
    assert_eq!(
        runner
            .menu
            .number_for_key("instruments.0.synth.ampEnv.releaseMs"),
        Some(185)
    );

    runner.menu.state.stack = vec![2, 0, 0, synth_group, oscillator_group, osc1_group];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux3", "delta": 1 }),
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
fn auto_map_fx_slot_uses_effect_specific_param_layout() {
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
fn auto_map_global_fx_covers_vinyl_params() {
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
fn fn_held_shows_auto_aux_mapping_overlay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
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
    assert_eq!(lines[4], "A4 Key");
    assert_eq!(runner.menu.state.stack, vec![2, 0, 0, 2, 2]);
    assert_eq!(runner.menu.state.cursor, 1);
}

#[test]
fn fn_held_shows_custom_aux_mapping_overlay_when_no_auto_map_applies() {
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
fn aux_overlay_waits_for_fn_hold_delay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
    runner.menu.state.cursor = 1;
    runner.ui.fn_held = true;
    runner.fn_hold_started_at = Some(Instant::now());

    let snapshot = runner.snapshot().unwrap();

    assert_ne!(snapshot["display"]["title"], "AUTO MAP");
}

#[test]
fn fn_aux_bind_sets_explicit_toast_and_marks_config_dirty() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 1];
    runner.menu.state.cursor = 0;
    runner.ui.fn_held = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
        })
        .unwrap();

    assert_eq!(
        runner.toast.as_ref().unwrap().message,
        "S1: Bound turn: Master Vol"
    );
    assert!(runner.config_dirty);
}

#[test]
fn aux_click_action_toast_is_shown_for_platform_effect_actions() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: None,
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
        })
        .unwrap();

    assert_eq!(runner.toast.as_ref().unwrap().message, "S1: MIDI Panic");
}
