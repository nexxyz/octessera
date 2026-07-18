use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}
fn base() -> GravityState {
    gravity_init(serde_json::json!({"spawnRatePct":0,"slideChancePct":100,"settleAge":8})).unwrap()
}
fn visible(state: &GravityState) -> Vec<bool> {
    gravity_render_model(state).cells
}

#[test]
fn menu_palette_normalize_and_serialization() {
    let menu = gravity_config_menu();
    assert_eq!(menu[0].key, "spawnRatePct");
    assert_eq!(menu[0].max, Some(100));
    assert_eq!(menu[1].key, "slideChancePct");
    assert_eq!(menu[2].key, "settleAge");
    assert_eq!(menu[2].max, Some(32));
    assert_eq!(menu[3].key, "gravityDir");
    assert_eq!(menu[4].key, "dropSand");
    assert_eq!(menu[5].key, "clearBottom");
    assert_eq!(menu[6].key, "invertGravity");
    let state = gravity_deserialize(serde_json::json!({"cells":[9,0],"age":[999],"gravityDir":"bad","tickCounter":5,"spawnRatePct":999,"slideChancePct":999,"settleAge":99,"triggerTypes":["activate"]})).unwrap();
    assert_eq!(state.cells[0], SAND);
    assert_eq!(state.age[0], u8::MAX);
    assert_eq!(state.gravity_dir, "down");
    assert_eq!(state.spawn_rate_pct, 100);
    assert_eq!(state.settle_age, 32);
    assert!(!state.trigger_types.contains(&CellTriggerType::Activate));
    assert_eq!(state.tick_counter, 0);
    let serialized = gravity_serialize(&state).unwrap();
    assert!(serialized.get("triggerTypes").is_none());
    assert!(serialized.get("tickCounter").is_none());
    assert_eq!(
        gravity_serialize(&gravity_deserialize(serialized.clone()).unwrap()).unwrap(),
        serialized
    );
    let model = gravity_render_model(&state);
    assert_eq!(model.name, "gravity");
    assert_eq!(model.palette.inactive, crate::palette::BLACK);
    assert_eq!(model.palette.active, [255, 220, 120]);
}

#[test]
fn duplicate_destinations_are_prevented_and_spawn_does_not_mask_vacated_origin() {
    let mut context = context();
    let mut state = base();
    state.cells[grid_index(0, 2)] = SAND;
    state.cells[grid_index(2, 2)] = SAND;
    state.cells[grid_index(0, 1)] = SAND;
    state.cells[grid_index(2, 1)] = SAND;
    state.cells[grid_index(3, 1)] = SAND;
    state.cells[grid_index(0, 0)] = SAND;
    state.cells[grid_index(1, 0)] = SAND;
    state.cells[grid_index(2, 0)] = SAND;
    state.cells[grid_index(3, 0)] = SAND;

    let ticked = gravity_on_tick(state, &mut context);

    assert_eq!(ticked.cells[grid_index(1, 1)], SAND);
    assert_eq!(ticked.cells[grid_index(0, 2)], SAND);

    let mut state = gravity_init(serde_json::json!({ "spawnRatePct": 100 })).unwrap();
    state.cells[grid_index(4, 7)] = SAND;
    let ticked = gravity_on_tick(state, &mut context);
    assert_eq!(ticked.cells[grid_index(4, 7)], EMPTY);
    assert_eq!(
        ticked.trigger_types[grid_index(4, 7)],
        CellTriggerType::Deactivate
    );
}

#[test]
fn grid_press_and_fall_non_wrapping_one_step() {
    let mut context = context();
    let state = base();
    let state = gravity_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_eq!(state.cells[grid_index(2, 3)], SAND);
    assert_eq!(
        state.trigger_types[grid_index(2, 3)],
        CellTriggerType::Activate
    );
    let ticked = gravity_on_tick(state, &mut context);
    assert_eq!(ticked.cells[grid_index(2, 2)], SAND);
    assert_eq!(ticked.cells[grid_index(2, 1)], EMPTY);
    assert_eq!(
        ticked.trigger_types[grid_index(2, 2)],
        CellTriggerType::Activate
    );
    assert_eq!(
        ticked.trigger_types[grid_index(2, 3)],
        CellTriggerType::Deactivate
    );
}

#[test]
fn slide_clear_bottom_invert_and_spawn_are_deterministic() {
    let mut context = context();
    let mut state = base();
    state.cells[grid_index(2, 2)] = SAND;
    state.cells[grid_index(2, 1)] = SAND;
    state.cells[grid_index(1, 0)] = SAND;
    state.cells[grid_index(2, 0)] = SAND;
    state.cells[grid_index(3, 0)] = SAND;
    let slid = gravity_on_tick(state, &mut context);
    assert!(slid.cells[grid_index(1, 1)] == SAND || slid.cells[grid_index(3, 1)] == SAND);
    let inverted = gravity_on_input(
        slid,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "invertGravity".into(),
        }),
        &mut context,
    );
    assert_eq!(inverted.gravity_dir, "left");
    assert!(!inverted.trigger_types.contains(&CellTriggerType::Activate));
    let mut cleared = inverted;
    cleared.cells[grid_index(0, 4)] = SAND;
    let cleared = gravity_on_input(
        cleared,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "clearBottom".into(),
        }),
        &mut context,
    );
    assert_eq!(cleared.cells[grid_index(0, 4)], EMPTY);
    assert_eq!(
        cleared.trigger_types[grid_index(0, 4)],
        CellTriggerType::Deactivate
    );
    let dropped = gravity_on_input(
        base(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "dropSand".into(),
        }),
        &mut context,
    );
    assert_eq!(
        dropped.cells.iter().filter(|c| **c == SAND).count(),
        GRID_WIDTH
    );
}

#[test]
fn default_self_sustains_and_leak_is_bounded() {
    let mut context = context();
    let mut state = gravity_init(serde_json::Value::Null).unwrap();
    let mut previous = visible(&state);
    let mut terminal_same = 1usize;
    let mut terminal_full = usize::from(previous.iter().all(|cell| *cell));
    let mut final_frames = Vec::new();
    for _ in 0..300 {
        state = gravity_on_tick(state, &mut context);
        let frame = visible(&state);
        terminal_same = if frame == previous {
            terminal_same + 1
        } else {
            1
        };
        terminal_full = if frame.iter().all(|cell| *cell) {
            terminal_full + 1
        } else {
            0
        };
        assert!(
            state
                .trigger_types
                .iter()
                .filter(|trigger| **trigger == CellTriggerType::Deactivate)
                .count()
                <= CELL_COUNT / 2
        );
        final_frames.push(frame.clone());
        if final_frames.len() > 16 {
            final_frames.remove(0);
        }
        previous = frame;
    }
    assert!(terminal_same <= 2);
    assert!(terminal_full <= 2);
    assert!(final_frames.windows(2).any(|window| window[0] != window[1]));
}
