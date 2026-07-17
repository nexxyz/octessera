use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

fn empty() -> CyclicState {
    cyclic_init(serde_json::json!({ "states": 4, "threshold": 2, "range": 1 })).unwrap()
}

#[test]
fn menu_palette_and_normalization_contract() {
    let menu = cyclic_config_menu();
    assert_eq!(menu[0].key, "states");
    assert_eq!(menu[0].min, Some(3));
    assert_eq!(menu[0].max, Some(8));
    assert_eq!(menu[1].key, "threshold");
    assert_eq!(menu[2].key, "range");
    assert_eq!(menu[3].key, "seedCycle");

    let state = cyclic_deserialize(serde_json::json!({
        "states": 999,
        "threshold": 999,
        "range": 999,
        "cells": [999, 8, 7],
        "ages": [999],
        "triggerTypes": ["activate"]
    }))
    .unwrap();
    assert_eq!(state.states, 8);
    assert_eq!(state.threshold, 8);
    assert_eq!(state.range, 2);
    assert_eq!(state.cells[0], 7);
    assert_eq!(state.cells[1], 0);
    assert_eq!(state.ages[0], u8::MAX);
    assert!(!state.trigger_types.contains(&CellTriggerType::Activate));

    let serialized = cyclic_serialize(&state).unwrap();
    assert!(serialized.get("triggerTypes").is_none());
    let restored = cyclic_deserialize(serialized.clone()).unwrap();
    assert_eq!(cyclic_serialize(&restored).unwrap(), serialized);

    let model = cyclic_render_model(&state);
    assert_eq!(model.name, "cyclic");
    assert_eq!(model.palette.inactive, crate::palette::BLACK);
    assert_eq!(model.palette.active, [255, 120, 220]);
}

#[test]
fn grid_press_exact_world_cell_and_wrap_deactivate() {
    let mut context = context();
    let state = empty();
    let state = cyclic_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_eq!(state.cells[grid_index(2, 3)], 1);
    assert_eq!(
        state.trigger_types[grid_index(2, 3)],
        CellTriggerType::Activate
    );

    let mut state = state;
    state.cells[grid_index(2, 3)] = state.states - 1;
    let state = cyclic_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_eq!(state.cells[grid_index(2, 3)], 0);
    assert_eq!(
        state.trigger_types[grid_index(2, 3)],
        CellTriggerType::Deactivate
    );
}

#[test]
fn threshold_advancement_and_non_wrapping_edges() {
    let mut context = context();
    let mut state = empty();
    state.cells[grid_index(0, 0)] = 1;
    state.cells[grid_index(0, 1)] = 2;
    state.cells[grid_index(1, 0)] = 2;
    let state = cyclic_on_tick(state, &mut context);
    assert_eq!(state.cells[grid_index(0, 0)], 2);
    assert_eq!(
        state.trigger_types[grid_index(0, 0)],
        CellTriggerType::Activate
    );
    assert_eq!(state.cells[grid_index(GRID_WIDTH - 1, 0)], 0);
}

#[test]
fn seed_cycle_is_deterministic_and_activates_nonzero_cells() {
    let mut context = context();
    let state = empty();
    let seeded = cyclic_on_input(
        state,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedCycle".into(),
        }),
        &mut context,
    );
    assert_eq!(seeded.cells[grid_index(2, 2)], 1);
    assert_eq!(seeded.cells[grid_index(3, 2)], 2);
    assert_eq!(
        seeded.trigger_types[grid_index(2, 2)],
        CellTriggerType::Activate
    );

    let seeded_again = cyclic_on_input(
        empty(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedCycle".into(),
        }),
        &mut context,
    );
    assert_eq!(seeded.cells, seeded_again.cells);
}

#[test]
fn persistent_nonzero_stable_and_no_stale_activate() {
    let mut context = context();
    let mut state = empty();
    state.cells[grid_index(4, 4)] = 1;
    let ticked = cyclic_on_tick(state, &mut context);
    assert_eq!(ticked.cells[grid_index(4, 4)], 1);
    assert_eq!(
        ticked.trigger_types[grid_index(4, 4)],
        CellTriggerType::Stable
    );
}
