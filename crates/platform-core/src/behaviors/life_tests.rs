use super::*;

#[test]
fn blinker_oscillates() {
    let mut state = init(Value::Null).unwrap();
    state.cells[grid_index(2, 3)] = true;
    state.cells[grid_index(3, 3)] = true;
    state.cells[grid_index(4, 3)] = true;
    let mut context = BehaviorContext::new(120.0);
    let next = on_tick(state, &mut context);
    assert!(next.cells[grid_index(3, 2)]);
    assert!(next.cells[grid_index(3, 3)]);
    assert!(next.cells[grid_index(3, 4)]);
    assert!(!next.cells[grid_index(2, 3)]);
    assert!(context.emitted_events.is_empty());
}

#[test]
fn twelve_live_cells_do_not_emit_direct_notes() {
    let mut state = init(Value::Null).unwrap();
    for (x, y) in [
        (1, 1),
        (2, 1),
        (1, 2),
        (2, 2),
        (5, 1),
        (6, 1),
        (5, 2),
        (6, 2),
        (1, 5),
        (2, 5),
        (1, 6),
        (2, 6),
    ] {
        state.cells[grid_index(x, y)] = true;
    }
    let mut context = BehaviorContext::new(120.0);

    let next = on_tick(state, &mut context);

    assert_eq!(next.cells.iter().filter(|cell| **cell).count(), 12);
    assert!(context.emitted_events.is_empty());
}

#[test]
fn block_is_stable_and_grid_press_toggles_cell() {
    let mut state = init(Value::Null).unwrap();
    for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
        state.cells[grid_index(x, y)] = true;
    }
    let next = on_tick(state.clone(), &mut BehaviorContext::new(120.0));
    assert_eq!(next.cells, state.cells);
    assert_eq!(
        next.trigger_types[grid_index(2, 2)],
        CellTriggerType::Stable
    );

    let mut context = BehaviorContext::new(120.0);
    let toggled = on_input(
        init(Value::Null).unwrap(),
        DeviceInput::GridPress { x: 2, y: 3 },
        &mut context,
    );
    assert!(toggled.cells[grid_index(2, 3)]);
    let toggled = on_input(toggled, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert!(!toggled.cells[grid_index(2, 3)]);
}

#[test]
fn render_config_and_serialization_match_contract() {
    let mut state = init(Value::Null).unwrap();
    state.cells[grid_index(1, 1)] = true;
    let model = render_model(&state);
    assert_eq!(model.name, "game of life");
    assert_eq!(model.status_line, "Gen 0");
    assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);

    let menu = config_menu(&state);
    assert_eq!(
        menu.iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec![
            "randomCellsPerTick",
            "randomTickInterval",
            "spawnStep",
            "spawnRandom"
        ]
    );

    let raw = serialize(&state).unwrap();
    assert_eq!(deserialize(raw).unwrap(), state);
}

#[test]
fn glider_moves_diagonally_after_four_generations() {
    let mut state = init(Value::Null).unwrap();
    for (x, y) in [(2, 1), (3, 2), (1, 3), (2, 3), (3, 3)] {
        state.cells[grid_index(x, y)] = true;
    }
    let mut context = BehaviorContext::new(120.0);
    for _ in 0..4 {
        state = on_tick(state, &mut context);
    }
    for (x, y) in [(3, 2), (4, 3), (2, 4), (3, 4), (4, 4)] {
        assert!(state.cells[grid_index(x, y)]);
    }
}
