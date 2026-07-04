use super::*;

#[test]
pub(crate) fn entering_l1_selects_active_part_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_part_index = 2;
    runner.menu.rebuild(runner.menu_config());

    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "L1: Life");
    assert_eq!(snapshot["selectedRow"], 2);
}

#[test]
pub(crate) fn entering_l2_selects_active_part_row_after_event_group() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_part_index = 2;
    runner.menu.rebuild(runner.menu_config());

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&entered);
    assert_eq!(snapshot["display"]["title"], "L2: Sense");
    assert_eq!(snapshot["selectedRow"], 3);
}

#[test]
pub(crate) fn l2_sense_exposes_aux_mappings_and_enterable_part_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let entered_l2 = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let lines = snapshot_from(&entered_l2)["display"]["lines"]
        .as_array()
        .unwrap()
        .clone();
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "  Aux Mappings"));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "  Events when ... On"));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> P1: life"));
    assert!(lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("P1:")));

    let part = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&part);
    assert_eq!(snapshot["display"]["title"], "L2: Sense/P1: life");
    let part_lines = snapshot["display"]["lines"].as_array().unwrap().clone();
    assert!(part_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("") == "> Scanning"));
    assert!(part_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").trim() == "Events"));
}

#[test]
pub(crate) fn l2_sense_scan_mode_edits_into_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![1, 4, 0];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l2"]["scanMode"],
        "scanning"
    );
}

#[test]
pub(crate) fn behavior_none_hides_step_config_and_reset_but_preserves_step_rate() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "none".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.algorithm_step_pulses = 48;
    runner.menu.rebuild(runner.menu_config());
    let l1_items = &runner.menu.root.children[0].children[0].children;

    assert!(contains_key_recursive(l1_items, "behaviorId"));
    assert!(contains_key_recursive(l1_items, "parts.0.name"));
    assert!(!contains_key_recursive(l1_items, "algorithmStep"));
    assert!(!contains_label(l1_items, "Reset"));
    let auto_map = runner.resolve_aux_auto_map("L1: None", Some("algorithmStep"), None);
    assert!(auto_map.iter().all(Option::is_none));

    runner.menu.focus_item_key("behaviorId");
    runner.menu.turn_key("behaviorId", 1);
    runner.commit_structural_draft_key("behaviorId").unwrap();

    assert_ne!(runner.behavior.id(), "none");
    assert_eq!(runner.algorithm_step_pulses, 48);
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
pub(crate) fn entering_part_row_updates_active_part_index() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.active_part_index, 2);
    assert_eq!(
        snapshot_from(&entered)["display"]["title"],
        "L1: Life/P3: life"
    );
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
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let entered = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
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
