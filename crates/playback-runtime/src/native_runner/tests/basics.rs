use super::*;

#[test]
pub(crate) fn unsupported_behavior_errors() {
    let error = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "unsupported".into(),
        ..NativeRunnerConfig::default()
    })
    .err()
    .unwrap();
    assert!(error.contains("unsupported native behavior `unsupported`"));
}

#[test]
pub(crate) fn checked_in_default_restores_sequencer_grid_state() {
    let payload: Value =
        serde_json::from_str(include_str!("../../../../../config/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let expected_active_part_index = payload["runtimeConfig"]["activePartIndex"]
        .as_u64()
        .unwrap_or(0) as usize;
    let expected_behavior_id = payload["runtimeConfig"]["parts"]
        .get(expected_active_part_index)
        .and_then(|part| part.get("l1"))
        .and_then(|l1| l1.get("behaviorId"))
        .and_then(Value::as_str)
        .unwrap_or("life")
        .to_string();

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.behavior.id(), expected_behavior_id);
    assert_eq!(runner.active_part_index, expected_active_part_index);
    assert!(runner
        .engine
        .model()
        .unwrap()
        .cells
        .iter()
        .any(|cell| *cell));
    assert_eq!(runner.instruments[0].kind, "synth");
    assert_eq!(runner.sense_parts[0].activate_slot, 0);
    assert_eq!(runner.sense_parts[0].activate_action, "note_on");
    assert!(runner.input_events_while_paused);
}

#[test]
pub(crate) fn checked_in_default_emits_life_and_scanned_drum_over_initial_steps() {
    let payload: Value =
        serde_json::from_str(include_str!("../../../../../config/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner.apply_config_payload(payload).unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();

    let first = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let second = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let mut notes = musical_note_ons(&first);
    notes.extend(musical_note_ons(&second));
    assert!(notes.iter().any(|(channel, _)| *channel == 0));
    assert!(notes.iter().any(|(channel, _)| *channel == 1));
}

#[test]
pub(crate) fn sequencer_behavior_is_native_and_paintable() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    let model = runner.engine.model().unwrap();
    assert_eq!(runner.behavior.id(), "sequencer");
    assert_eq!(model.name, "sequencer");
    assert_eq!(model.status_line, "Manual");
    assert!(model.cells[platform_core::grid_index(2, 3)]);
}

#[test]
pub(crate) fn keys_behavior_reports_momentary_grid_interaction() {
    let runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    assert_eq!(runner.snapshot().unwrap()["gridInteraction"], "momentary");
}

#[test]
pub(crate) fn fresh_native_runner_uses_initial_sense_defaults() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert_eq!(runner.sense_parts[0].scan_mode, "none");
    assert_eq!(runner.sense_parts[0].scan_axis, "columns");
    assert_eq!(runner.sense_parts[0].scan_unit, "1/16");
    assert!(runner.sense_parts[0].event_enabled);
    assert!(!runner.sense_parts[1].event_enabled);
    assert_eq!(runner.sense_parts[0].lowest_note, 36);
    assert_eq!(runner.sense_parts[0].starting_note, 60);
    assert_eq!(runner.sense_parts[0].highest_note, 74);
    assert_eq!(runner.sense_parts[0].scale, "major_pentatonic");
    assert_eq!(runner.sense_parts[0].root, "D");
    assert_eq!(runner.sense_parts[0].out_of_range, "clamp");
    assert_eq!(runner.sense_parts[0].x_pitch_steps, 0);
    assert_eq!(runner.sense_parts[0].y_pitch_steps, 1);
    assert_eq!(runner.ui.master_volume, 73);
    assert_eq!(runner.global_sound.note_length_ms, 120);
    assert!(!runner.auto_save_default);
    assert!(runner.trigger_probability_maps[0]
        .iter()
        .all(|cell| cell == "full"));
}

#[test]
pub(crate) fn behavior_menu_actions_dispatch_selected_action_type() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "ant".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let action_cursor = runner.menu.root.children[0].children[0]
        .children
        .iter()
        .position(|item| {
            matches!(
                &item.value,
                crate::native_menu::NativeMenuValue::Action(
                    NativeMenuAction::BehaviorAction(action_type)
                ) if action_type == "spawnAnt"
            )
        })
        .expect("spawnAnt action row");
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = action_cursor;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    let model = runner.engine.model().unwrap();
    assert_eq!(model.name, "ant");
    assert!(model.cells.iter().any(|cell| *cell));
}

#[test]
pub(crate) fn transport_tick_returns_status_and_snapshot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    assert!(matches!(
        messages.last(),
        Some(RunnerMessage::RuntimeStatus { .. })
    ));
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
pub(crate) fn button_s_toggles_transport() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(matches!(
        messages.last(),
        Some(RunnerMessage::RuntimeStatus { status }) if status.transport == RuntimeTransportState::Playing
    ));

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(matches!(
        messages.last(),
        Some(RunnerMessage::RuntimeStatus { status }) if status.transport == RuntimeTransportState::Paused
    ));
}
