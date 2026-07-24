use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

fn empty() -> CyclicState {
    cyclic_deserialize(serde_json::json!({ "states": 4, "threshold": 2, "range": 1, "cells": [] }))
        .unwrap()
}

#[test]
fn empty_config_seeds_bounded_default_activity_and_deserialize_empty_stays_empty() {
    let mut context = context();
    let mut state = cyclic_init(serde_json::json!({})).unwrap();
    assert!(state.cells.iter().any(|cell| *cell != 0));
    assert!(state.cells.contains(&0));
    assert!(!state.trigger_types.contains(&CellTriggerType::Activate));
    let restored = cyclic_deserialize(serde_json::json!({ "cells": [] })).unwrap();
    assert!(restored.cells.iter().all(|cell| *cell == 0));
    let initialized_empty = cyclic_init(serde_json::json!({ "cells": [] })).unwrap();
    assert!(initialized_empty.cells.iter().all(|cell| *cell == 0));
    let missing_state = cyclic_deserialize(serde_json::json!({})).unwrap();
    assert!(missing_state.cells.iter().any(|cell| *cell != 0));
    let mut max_activates = 0;
    let mut total_activates = 0;
    for _ in 0..128 {
        state = cyclic_on_tick(state, &mut context);
        let activates = state
            .trigger_types
            .iter()
            .filter(|trigger| **trigger == CellTriggerType::Activate)
            .count();
        max_activates = max_activates.max(activates);
        total_activates += activates;
    }
    assert!(total_activates > 0);
    assert!(max_activates <= 48);
    assert!(total_activates <= 384);
}

#[test]
fn default_seed_stays_non_static_and_non_full_over_detector_window() {
    let mut context = context();
    let mut state = cyclic_init(serde_json::json!({})).unwrap();
    let mut static_frames = 0;
    for _ in 0..300 {
        let previous = state.cells.clone();
        state = cyclic_on_tick(state, &mut context);
        if state.cells == previous {
            static_frames += 1;
        } else {
            static_frames = 0;
        }
        assert!(static_frames <= 2);
        assert!(state.cells.contains(&0));
    }
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
    assert_eq!(seeded.cells[grid_index(2, 2)], 0);
    assert_eq!(seeded.cells[grid_index(3, 2)], 1);
    assert_eq!(
        seeded.trigger_types[grid_index(3, 2)],
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

#[test]
fn seed_cycle_rotates_exact_valid_patterns_for_every_state_count() {
    let cycle = [
        grid_index(2, 2),
        grid_index(3, 2),
        grid_index(2, 3),
        grid_index(3, 3),
    ];
    let base = [0, 1, 3, 2];
    let mut context = context();
    for states in 3..=8 {
        for rotation in 0..states {
            let mut state = cyclic_deserialize(serde_json::json!({
                "states": states, "threshold": 8, "range": 1, "cells": []
            }))
            .unwrap();
            state.ages.fill(9);
            for (position, index) in cycle.iter().enumerate() {
                state.cells[*index] = (base[position] + rotation) % states;
            }

            let next = cyclic_on_tick(state, &mut context);

            for (position, index) in cycle.iter().enumerate() {
                assert_eq!(next.cells[*index], (base[position] + rotation + 1) % states);
                assert_eq!(next.ages[*index], 0);
            }
        }
    }
}

#[test]
fn unrelated_four_cell_state_does_not_enter_seed_cycle_path() {
    let mut state = cyclic_deserialize(serde_json::json!({
        "states": 4, "threshold": 8, "range": 1, "cells": []
    }))
    .unwrap();
    let cycle = [
        grid_index(2, 2),
        grid_index(3, 2),
        grid_index(2, 3),
        grid_index(3, 3),
    ];
    for (value, index) in [0, 1, 2, 3].into_iter().zip(cycle) {
        state.cells[index] = value;
    }

    let next = cyclic_on_tick(state, &mut context());

    assert_eq!(cycle.map(|index| next.cells[index]), [0, 1, 2, 3]);
    assert!(cycle.iter().all(|index| next.ages[*index] == 1));
}

#[test]
fn state_advancement_marks_each_local_nonzero_change_as_activate() {
    let center = grid_index(4, 4);
    let neighbor = grid_index(5, 4);
    let mut state = cyclic_deserialize(serde_json::json!({
        "states": 4, "threshold": 1, "range": 1, "cells": []
    }))
    .unwrap();
    state.cells[center] = 1;
    state.cells[neighbor] = 2;

    let first = cyclic_on_tick(state, &mut context());
    assert_eq!(first.cells[center], 2);
    assert_eq!(first.trigger_types[center], CellTriggerType::Activate);

    let mut second_input = first;
    second_input.cells[neighbor] = 3;
    let second = cyclic_on_tick(second_input, &mut context());
    assert_eq!(second.cells[center], 3);
    assert_eq!(second.trigger_types[center], CellTriggerType::Activate);
}

#[test]
fn malformed_transient_data_preserves_persistent_fields_and_clamps_below_minimum() {
    let state = cyclic_deserialize(serde_json::json!({
        "cells": [1, 2],
        "ages": [7],
        "triggerTypes": ["not-a-trigger"],
        "states": -1,
        "threshold": -2,
        "range": -3
    }))
    .unwrap();

    assert_eq!(state.cells[0], 1);
    assert_eq!(state.cells[1], 2);
    assert_eq!(state.ages[0], 7);
    assert_eq!(state.states, 3);
    assert_eq!(state.threshold, 1);
    assert_eq!(state.range, 1);
    assert!(!state.trigger_types.contains(&CellTriggerType::Activate));
}
