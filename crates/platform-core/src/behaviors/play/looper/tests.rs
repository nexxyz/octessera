use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn init_exposes_overdub_length_and_clear_menu() {
    let state = looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 4 })).unwrap();
    assert_eq!(state.mode.as_str(), "overdub");
    assert_eq!(looper_init(Value::Null).unwrap().mode.as_str(), "overdub");
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
    assert!(state.tiles[index]);
    assert!(state.held_tiles[index]);
    assert!(state.steps.iter().all(Vec::is_empty));

    let state = looper_on_input(state, DeviceInput::GridRelease { x: 1, y: 2 }, &mut context);
    assert!(!state.tiles[index]);
    assert_eq!(state.trigger_types[index], TileTriggerType::Deactivate);
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
    assert!(!state.playback_tiles[index]);
    assert!(!state.tiles[index]);
    assert_eq!(state.trigger_types[index], TileTriggerType::Deactivate);

    state = looper_on_tick(state, &mut context);
    assert!(state.playback_tiles[index]);
    assert!(state.tiles[index]);
    assert_eq!(state.trigger_types[index], TileTriggerType::Activate);
}

#[test]
fn outside_notes_record_into_the_sounding_step_without_holding_the_tile() {
    let mut context = context();
    let mut state =
        looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 4 })).unwrap();
    state = looper_on_tick(state, &mut context);
    state = looper_on_tick(state, &mut context);
    let tile = grid_index(1, 2);

    assert!(record_outside_tile(&mut state, tile, true));
    assert!(record_outside_tile(&mut state, tile, false));
    assert!(!state.held_tiles[tile] && !state.tiles[tile]);
    let press = LooperEvent {
        tile,
        kind: LooperEventKind::Press,
    };
    let release = LooperEvent {
        tile,
        kind: LooperEventKind::Release,
    };
    assert_eq!(state.steps[1], vec![press]);
    assert_eq!(
        state.steps[2],
        vec![release],
        "a same-step release moves on"
    );

    for _ in 0..3 {
        state = looper_on_tick(state, &mut context);
        assert!(!state.tiles[tile], "this pass already sounded the note");
    }
    state = looper_on_tick(state, &mut context);
    assert!(state.tiles[tile], "the next pass plays it");
    assert_eq!(state.trigger_types[tile], TileTriggerType::Activate);

    state.mode = LooperMode::Play;
    assert!(!record_outside_tile(&mut state, tile, true));
}

#[test]
fn clear_loop_preserves_live_holds_and_releases_playback_only_tiles() {
    let mut context = context();
    let mut state =
        looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 2 })).unwrap();
    let live = grid_index(1, 1);
    let looped = grid_index(2, 2);
    state.held_tiles[live] = true;
    state.playback_tiles[looped] = true;
    state.tiles[live] = true;
    state.tiles[looped] = true;
    state.steps[0].push(LooperEvent {
        tile: looped,
        kind: LooperEventKind::Press,
    });

    let state = looper_on_input(
        state,
        DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
            action_type: "clearLoop".into(),
        }),
        &mut context,
    );
    assert!(state.tiles[live]);
    assert!(!state.tiles[looped]);
    assert_eq!(state.trigger_types[looped], TileTriggerType::Deactivate);
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
    assert!(serialized.get("heldTiles").is_none());
    assert!(serialized.get("playbackTiles").is_none());
    assert_eq!(serialized["steps"][0].as_array().unwrap().len(), 1);

    let restored = looper_deserialize(serialized).unwrap();
    assert!(restored.tiles.iter().all(|tile| !tile));
    assert!(restored.held_tiles.iter().all(|tile| !tile));
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
    assert_eq!(state.mode.as_str(), "play");
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
    assert_eq!(state.mode.as_str(), "overdub");
    assert_eq!(state.steps[0].len(), 1);
}

#[test]
fn toggle_mode_action_switches_recording_without_clearing_sequence_or_phase() {
    let mut context = context();
    let mut state = looper_init(Value::Null).unwrap();
    state.step_index = 1;
    state.steps[0].push(LooperEvent {
        tile: grid_index(1, 1),
        kind: LooperEventKind::Press,
    });
    state = looper_on_input(
        state,
        DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
            action_type: "toggleMode".into(),
        }),
        &mut context,
    );
    assert_eq!(state.mode.as_str(), "play");
    assert_eq!(state.step_index, 1);
    assert_eq!(state.steps[0].len(), 1);

    state = looper_on_input(
        state,
        DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
            action_type: "toggleMode".into(),
        }),
        &mut context,
    );
    assert_eq!(state.mode.as_str(), "overdub");
    assert_eq!(state.step_index, 1);
    assert_eq!(state.steps[0].len(), 1);
}

#[test]
fn reset_transport_phase_restarts_playback_and_preserves_loop_state() {
    let mut state = looper_init(serde_json::json!({
        "mode": "play",
        "lengthSteps": 4
    }))
    .unwrap();
    let live = grid_index(1, 1);
    let looped = grid_index(2, 2);
    state.held_tiles[live] = true;
    state.playback_tiles[looped] = true;
    state.tiles = combined_tiles(&state.held_tiles, &state.playback_tiles);
    state.step_index = 3;
    state.steps[2].push(LooperEvent {
        tile: looped,
        kind: LooperEventKind::Press,
    });
    let steps = state.steps.clone();
    let held_tiles = state.held_tiles.clone();

    reset_transport_phase(&mut state);

    assert_eq!(state.step_index, 0);
    assert!(state.playback_tiles.iter().all(|tile| !tile));
    assert_eq!(state.tiles, held_tiles);
    assert_eq!(state.trigger_types[looped], TileTriggerType::Deactivate);
    assert_eq!(state.trigger_types[live], TileTriggerType::Stable);
    assert_eq!(state.steps, steps);
    assert_eq!(state.mode.as_str(), "play");
    assert_eq!(state.length_steps, 4);
    assert_eq!(state.held_tiles, held_tiles);
}

#[test]
fn saved_looper_modes_load_and_save_back_unchanged() {
    for mode in ["overdub", "play"] {
        let state = looper_deserialize(serde_json::json!({ "mode": mode })).unwrap();
        assert_eq!(looper_serialize(&state).unwrap()["mode"], mode);
        assert_eq!(serde_json::to_value(&state).unwrap()["mode"], mode);
    }
    for unknown in [serde_json::json!("Play"), serde_json::json!(3)] {
        let state = looper_deserialize(serde_json::json!({ "mode": unknown })).unwrap();
        assert_eq!(state.mode, LooperMode::Overdub);
    }
}

fn recorded_two_step_loop() -> LooperState {
    let mut context = context();
    let mut state =
        looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 4 })).unwrap();
    state = looper_on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut context);
    for _ in 0..3 {
        state = looper_on_tick(state, &mut context);
    }
    looper_on_input(state, DeviceInput::GridPress { x: 1, y: 1 }, &mut context)
}

fn with_length(state: &LooperState, length: usize) -> LooperState {
    let mut saved = looper_serialize(state).unwrap();
    saved["lengthSteps"] = serde_json::json!(length);
    looper_deserialize(saved).unwrap()
}

#[test]
fn lowering_length_keeps_steps_and_raising_restores_them() {
    let state = recorded_two_step_loop();
    assert_eq!(state.steps[3].len(), 1);
    let shorter = with_length(&state, 2);
    assert_eq!(shorter.length_steps, 2);
    assert_eq!(shorter.steps[3].len(), 1);
    let restored = with_length(&shorter, 4);
    assert_eq!(restored.steps, state.steps);
}

#[test]
fn playback_and_recording_stay_inside_the_length_window() {
    let mut context = context();
    let mut state = with_length(&recorded_two_step_loop(), 2);
    let hidden = grid_index(1, 1);
    for _ in 0..8 {
        state = looper_on_tick(state, &mut context);
        assert!(state.step_index < 2);
        assert!(!state.playback_tiles[hidden]);
    }
    state = looper_on_input(state, DeviceInput::GridPress { x: 2, y: 2 }, &mut context);
    assert!(state.steps[state.step_index]
        .iter()
        .any(|event| event.tile == grid_index(2, 2)));
    assert_eq!(state.steps[3].len(), 1);
}

#[test]
fn clear_loop_also_clears_steps_beyond_length() {
    let mut context = context();
    let state = with_length(&recorded_two_step_loop(), 2);
    let state = looper_on_input(
        state,
        DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
            action_type: "clearLoop".into(),
        }),
        &mut context,
    );
    assert!(state.steps.iter().all(Vec::is_empty));
}
