use super::*;

#[test]
pub(crate) fn entering_worlds_selects_active_layer_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_layer_index = 2;
    runner.menu.rebuild(runner.menu_config());

    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "/Build");
    assert_eq!(snapshot["selectedRow"], 2);
}

#[test]
pub(crate) fn entering_pulses_selects_active_layer_row_after_event_group() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_layer_index = 2;
    runner.menu.rebuild(runner.menu_config());

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "/Link");
    assert_eq!(snapshot["selectedRow"], 3);
}

#[test]
pub(crate) fn pulses_exposes_aux_mappings_and_enterable_layer_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let layer_index = runner.menu.root.children[1]
        .children
        .iter()
        .position(|item| item.label.starts_with("L1:"))
        .expect("layer row");
    runner.menu.state.cursor = layer_index;
    let lines = runner.snapshot().unwrap()["display"]["lines"]
        .as_array()
        .unwrap()
        .clone();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "  Aux Mappings >"));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "  LFOs >"));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("L1:")));

    assert!(runner.menu.focus_item_key("layers.0.pulses.scanMode"));
    let snapshot = runner.snapshot().unwrap();
    assert!(snapshot["display"]["title"]
        .as_str()
        .unwrap_or_default()
        .contains("L1: life"));
    let layer_lines = snapshot["display"]["lines"].as_array().unwrap().clone();
    assert!(layer_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("Scan Mode")));
}

#[test]
pub(crate) fn pulses_scan_mode_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("layers.0.pulses.scanMode"));

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["pulses"]["scanMode"],
        "scanning"
    );
    assert!(contains_key_recursive(
        &runner.menu.root.children,
        "layers.0.pulses.scanAxis"
    ));
}

#[test]
pub(crate) fn behavior_none_hides_step_config_and_reset_but_preserves_step_rate() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "none".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.algorithm_step_pulses = 48;
    runner.menu.rebuild(runner.menu_config());
    let worlds_items = &runner.menu.root.children[0].children[0].children;

    assert!(contains_key_recursive(worlds_items, "behaviorId"));
    assert!(contains_key_recursive(worlds_items, "layers.0.name"));
    assert!(!contains_key_recursive(worlds_items, "algorithmStep"));
    assert!(!contains_label(worlds_items, "Reset"));
    let auto_map = runner.resolve_aux_auto_map("1: None", Some("algorithmStep"), None);
    assert!(auto_map.iter().all(Option::is_none));

    select_behavior(&mut runner, "life");

    assert_ne!(runner.behavior.id(), "none");
    assert_eq!(runner.transport.algorithm_step_pulses, 48);
    assert!(contains_key_recursive(
        &runner.menu.root.children[0].children[0].children,
        "algorithmStep"
    ));
}

#[test]
pub(crate) fn instrument_none_hides_note_mode_params_and_slot_actions() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "none".into();
    runner.menu.rebuild(runner.menu_config());
    let instrument_items = &runner.menu.root.children[2].children[0].children[0].children;

    assert!(contains_key_recursive(
        instrument_items,
        "instruments.0.type"
    ));
    assert!(contains_key_recursive(
        instrument_items,
        "instruments.0.name"
    ));
    assert!(!contains_key_recursive(
        instrument_items,
        "instruments.0.noteBehavior"
    ));
    assert!(!contains_label(instrument_items, "Synth"));
    assert!(!contains_label(instrument_items, "Sampler"));
    assert!(!contains_label(instrument_items, "MIDI"));
    assert!(!contains_label(instrument_items, "Mixer"));
    assert!(!contains_label(instrument_items, "Slot Actions"));
}

#[test]
pub(crate) fn entering_layer_row_updates_active_layer_index() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.active_layer_index, 2);
    assert_eq!(snapshot_from(&entered)["display"]["title"], "/B/L3: life");
}

#[test]
pub(crate) fn instrument_list_shows_compact_name_labels() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[1].kind = "sampler".into();
    runner.instruments[1].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    let lines = snapshot_from(&entered)["display"]["lines"]
        .as_array()
        .unwrap()
        .clone();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("I1: synth")));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("I2: samp direct")));
}
