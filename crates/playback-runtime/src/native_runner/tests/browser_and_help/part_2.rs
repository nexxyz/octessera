use super::*;

#[test]
pub(crate) fn fn_shift_enter_opens_contextual_help_and_enter_closes_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_shift", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let modifiers = snapshot_from(
        &runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "button_combined_modifier", "pressed": true }),
                request_snapshot: None,
            })
            .unwrap(),
    );
    assert_eq!(modifiers["settings"]["shiftHeld"], false);
    assert_eq!(modifiers["settings"]["fnHeld"], false);
    assert_eq!(modifiers["settings"]["combinedModifierHeld"], true);
    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let snapshot = snapshot_from(&opened);
    assert_eq!(snapshot["display"]["title"], "Help: Instruments");
    let help_lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(help_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("destination")));
    assert_eq!(help_lines.last().unwrap(), "> Close");

    let closed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(snapshot_from(&closed)["display"]["title"], "L3: Voice");
}

#[test]
pub(crate) fn submenu_snapshot_does_not_append_transport_status_line() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![1];
    let messages = runner.messages_with_snapshot().unwrap();
    let lines = snapshot_from(&messages)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert!(lines.iter().all(|line| !line.contains("Stopped")));
}

#[test]
pub(crate) fn instrument_pan_menu_edit_moves_monotonically_from_current_value() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].pan_pos = 10;
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 3];
    runner.menu.state.cursor = 2;
    runner.menu.state.editing = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.instruments[0].pan_pos, 11);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.instruments[0].pan_pos, 12);
}

#[test]
pub(crate) fn active_part_trigger_toggle_suppresses_and_restores_with_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.ui.fn_held = true;
    let off = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "zero");
    assert_eq!(snapshot_from(&off)["display"]["toast"], "P1 triggers off");

    let on = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "full");
    assert_eq!(snapshot_from(&on)["display"]["toast"], "P1 triggers full");
}

#[test]
pub(crate) fn repeated_autosaves_increment_flash_serial() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    let first = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    let first_serial = snapshot_from(&first)["settings"]["autoSaveFlashSerial"]
        .as_u64()
        .unwrap();

    let second = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    let second_serial = snapshot_from(&second)["settings"]["autoSaveFlashSerial"]
        .as_u64()
        .unwrap();
    assert!(second_serial > first_serial);
}

#[test]
pub(crate) fn config_load_queues_midi_port_selection_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "midi": {
                    "enabled": true,
                    "outId": "out1",
                    "inId": "in1"
                }
            }
        }))
        .unwrap();

    let messages = runner.messages_with_snapshot().unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![
                RuntimePlatformEffect::MidiSelectOutput { id: Some("out1".into()) },
                RuntimePlatformEffect::MidiSelectInput { id: Some("in1".into()) },
            ]
    )));
}

#[test]
pub(crate) fn contextual_help_includes_midi_output_guidance() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 3];
    runner.menu.state.cursor = 2;
    runner.ui.combined_modifier_held = true;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let lines = snapshot_from(&opened)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(lines.contains("output"));
}

#[test]
pub(crate) fn contextual_help_resolves_life_params_and_scrolls_to_bottom() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        behavior_config: json!({ "randomCellsPerTick": 12, "randomTickInterval": 1 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 4;
    runner.ui.combined_modifier_held = true;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let lines = snapshot_from(&opened)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(lines.contains("random cells"));
    runner.ui.combined_modifier_held = false;

    runner.turn_help_popup(99);
    let help = runner.help_popup.as_ref().unwrap();
    assert_eq!(
        help.scroll,
        help.lines.len().saturating_sub(OLED_BODY_ROWS - 1)
    );
}
