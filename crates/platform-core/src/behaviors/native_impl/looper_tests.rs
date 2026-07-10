use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn init_exposes_overdub_length_and_clear_menu() {
    let state = looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 4 })).unwrap();
    assert_eq!(state.mode, "overdub");
    assert_eq!(looper_init(Value::Null).unwrap().mode, "overdub");
    assert_eq!(state.length_steps, 4);
    assert_eq!(state.steps.len(), 4);
    let menu = looper_config_menu();
    assert_eq!(menu[0].key, "toggleMode");
    assert_eq!(menu[0].label, "Punch In/Out");
    assert_eq!(menu[1].key, "lengthSteps");
    assert_eq!(menu[2].key, "clearLoop");
    assert_eq!(
        grid_interaction_for_looper(),
        Some(GridInteraction::Momentary)
    );
}

#[test]
fn play_mode_behaves_like_keys_without_recording() {
    let mut context = context();
    let state = looper_init(serde_json::json!({ "mode": "play" })).unwrap();
    let state = looper_on_input(state, DeviceInput::GridPress { x: 1, y: 2 }, &mut context);
    let index = grid_index(1, 2);
    assert!(state.cells[index]);
    assert!(state.held_cells[index]);
    assert!(state.steps.iter().all(Vec::is_empty));

    let state = looper_on_input(state, DeviceInput::GridRelease { x: 1, y: 2 }, &mut context);
    assert!(!state.cells[index]);
    assert_eq!(state.trigger_types[index], CellTriggerType::Deactivate);
}

#[test]
fn overdub_records_and_replays_press_release_events() {
    let mut context = context();
    let mut state =
        looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 2 })).unwrap();
    state = looper_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    let index = grid_index(2, 3);
    assert_eq!(state.steps[0].len(), 1);
    state = looper_on_tick(state, &mut context);
    state = looper_on_input(state, DeviceInput::GridRelease { x: 2, y: 3 }, &mut context);
    assert_eq!(state.steps[1].len(), 1);

    state = looper_on_tick(state, &mut context);
    assert!(!state.playback_cells[index]);
    assert!(!state.cells[index]);
    assert_eq!(state.trigger_types[index], CellTriggerType::Deactivate);

    state = looper_on_tick(state, &mut context);
    assert!(state.playback_cells[index]);
    assert!(state.cells[index]);
    assert_eq!(state.trigger_types[index], CellTriggerType::Activate);
}

#[test]
fn clear_loop_preserves_live_holds_and_releases_playback_only_cells() {
    let mut context = context();
    let mut state =
        looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 2 })).unwrap();
    let live = grid_index(1, 1);
    let looped = grid_index(2, 2);
    state.held_cells[live] = true;
    state.playback_cells[looped] = true;
    state.cells[live] = true;
    state.cells[looped] = true;
    state.steps[0].push(LooperEvent {
        cell: looped,
        kind: LooperEventKind::Press,
    });

    let state = looper_on_input(
        state,
        DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
            action_type: "clearLoop".into(),
        }),
        &mut context,
    );
    assert!(state.cells[live]);
    assert!(!state.cells[looped]);
    assert_eq!(state.trigger_types[looped], CellTriggerType::Deactivate);
    assert!(state.steps.iter().all(Vec::is_empty));
}

#[test]
fn serialized_state_stores_sequence_without_live_holds() {
    let mut context = context();
    let state = looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 2 })).unwrap();
    let state = looper_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    let serialized = looper_serialize(&state).unwrap();
    assert_eq!(serialized["mode"], "overdub");
    assert_eq!(serialized["lengthSteps"], 2);
    assert!(serialized.get("stepIndex").is_none());
    assert!(serialized.get("heldCells").is_none());
    assert!(serialized.get("playbackCells").is_none());
    assert_eq!(serialized["steps"][0].as_array().unwrap().len(), 1);

    let restored = looper_deserialize(serialized).unwrap();
    assert!(restored.cells.iter().all(|cell| !cell));
    assert!(restored.held_cells.iter().all(|cell| !cell));
    assert_eq!(restored.steps[0].len(), 1);
}

#[test]
fn set_mode_action_switches_recording_without_clearing_sequence() {
    let mut context = context();
    let mut state = looper_init(Value::Null).unwrap();
    state = looper_on_input(
        state,
        DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
            action_type: "setMode:play".into(),
        }),
        &mut context,
    );
    assert_eq!(state.mode, "play");
    state = looper_on_input(state, DeviceInput::GridPress { x: 1, y: 1 }, &mut context);
    assert!(state.steps.iter().all(Vec::is_empty));

    state = looper_on_input(
        state,
        DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
            action_type: "setMode:overdub".into(),
        }),
        &mut context,
    );
    state = looper_on_input(state, DeviceInput::GridPress { x: 2, y: 2 }, &mut context);
    assert_eq!(state.mode, "overdub");
    assert_eq!(state.steps[0].len(), 1);
}

#[test]
fn toggle_mode_action_switches_recording_without_clearing_sequence_or_phase() {
    let mut context = context();
    let mut state = looper_init(Value::Null).unwrap();
    state.step_index = 1;
    state.steps[0].push(LooperEvent {
        cell: grid_index(1, 1),
        kind: LooperEventKind::Press,
    });
    state = looper_on_input(
        state,
        DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
            action_type: "toggleMode".into(),
        }),
        &mut context,
    );
    assert_eq!(state.mode, "play");
    assert_eq!(state.step_index, 1);
    assert_eq!(state.steps[0].len(), 1);

    state = looper_on_input(
        state,
        DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
            action_type: "toggleMode".into(),
        }),
        &mut context,
    );
    assert_eq!(state.mode, "overdub");
    assert_eq!(state.step_index, 1);
    assert_eq!(state.steps[0].len(), 1);
}
