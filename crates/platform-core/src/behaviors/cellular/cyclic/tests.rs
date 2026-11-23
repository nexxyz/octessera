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
    crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
        let mut context = context();
        let mut state = cyclic_init(serde_json::json!({})).unwrap();
        assert!(state.cells.iter().any(|cell| *cell != 0));
        assert!(state.cells.contains(&0));
        assert!(!state.trigger_types.contains(&TileTriggerType::Activate));
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
                .filter(|trigger| **trigger == TileTriggerType::Activate)
                .count();
            max_activates = max_activates.max(activates);
            total_activates += activates;
        }
        assert!(total_activates > 0);
        assert!(max_activates < TILE_COUNT);
    });
}

#[test]
fn default_seed_stays_non_static_and_non_full_over_detector_window() {
    crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
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
    });
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
    assert!(!state.trigger_types.contains(&TileTriggerType::Activate));

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
fn every_press_lights_its_tile_as_a_front() {
    let mut context = context();
    let mut state = empty();
    for cell in [0, 1, 2, state.states - 1] {
        state.cells[grid_index(2, 3)] = cell;
        state = cyclic_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        assert_eq!(state.cells[grid_index(2, 3)], FRONT, "from {cell}");
        assert_eq!(
            state.trigger_types[grid_index(2, 3)],
            TileTriggerType::Activate,
            "from {cell}"
        );
    }
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
        TileTriggerType::Deactivate
    );
    assert_eq!(state.cells[grid_index(GRID_WIDTH - 1, 0)], 0);
}

#[test]
fn seed_cycle_scatters_a_fresh_mix_of_every_state() {
    crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
        let mut context = context();
        let seed = |context: &mut BehaviorContext| {
            cyclic_on_input(
                empty(),
                DeviceInput::BehaviorAction(BehaviorActionInput {
                    action_type: "seedCycle".into(),
                }),
                context,
            )
        };
        let seeded = seed(&mut context);
        for value in 0..seeded.states {
            assert!(seeded.cells.contains(&value), "state {value} missing");
        }
        let lit = seeded.cells.iter().position(|cell| *cell == FRONT).unwrap();
        assert_eq!(seeded.trigger_types[lit], TileTriggerType::Activate);
        assert_ne!(seeded.cells, seed(&mut context).cells);
    });
}

#[test]
fn a_frozen_world_seeds_itself_again() {
    crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
        let mut context = context();
        let mut state = cyclic_deserialize(serde_json::json!({
            "states": 8, "threshold": 8, "range": 1, "cells": []
        }))
        .unwrap();
        let reseeded = (0..8 * STALL_TICKS).any(|_| {
            state = cyclic_on_tick(state.clone(), &mut context);
            state.ages.iter().all(|age| *age == 0)
                && state.cells.iter().filter(|cell| **cell != 0).count() > 8
        });
        assert!(reseeded);
    });
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
        TileTriggerType::Stable
    );
}

#[test]
fn cells_without_a_successor_neighbor_stay_and_age() {
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
    for (value, index) in [0, 1, 2, 0].into_iter().zip(cycle) {
        state.cells[index] = value;
    }

    let next = cyclic_on_tick(state, &mut context());

    let moved = cycle
        .iter()
        .zip([0, 1, 2, 0])
        .filter(|(index, value)| next.cells[**index] != *value);
    assert!(moved.count() <= 1, "only the stray cell may change");
    assert!(cycle.iter().all(|index| next.ages[*index] == 1));
}

#[test]
fn at_range_one_every_neighbor_counts_once() {
    let mut state = empty();
    for (x, y) in [(4, 5), (3, 4), (5, 4), (3, 5)] {
        state.cells[grid_index(x, y)] = 1;
    }
    assert_eq!(count_neighbors(&state.cells, 4, 4, 1, 1), 4);
    assert_eq!(count_neighbors(&state.cells, 4, 4, 1, 2), 4);
}

#[test]
fn fewer_successors_than_the_threshold_move_a_cell_on_only_sometimes() {
    crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
        let mut state = cyclic_deserialize(serde_json::json!({
            "states": 4, "threshold": 4, "range": 1, "cells": []
        }))
        .unwrap();
        state.cells[grid_index(4, 5)] = 1;
        let center = grid_index(4, 4);
        let far = grid_index(0, 0);
        let mut moved = 0;
        for _ in 0..200 {
            let next = cyclic_on_tick(state.clone(), &mut context());
            moved += usize::from(next.cells[center] == 1);
            assert_eq!(next.cells[far], 0, "a cell with no successor waits");
        }
        assert!((20..80).contains(&moved), "{moved} of 200");
    });
}

#[test]
fn at_threshold_one_the_spirals_turn_with_period_states_and_keep_moving() {
    crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
        let mut context = context();
        let mut state = cyclic_init(serde_json::json!({})).unwrap();
        let period = usize::from(state.states);
        let mut history = Vec::new();
        for _ in 0..300 {
            state = cyclic_on_tick(state, &mut context);
            history.push(state.cells.clone());
        }
        let tail = &history[150..];
        assert!(tail[period..].iter().zip(tail).all(|(a, b)| a == b));
        assert!(tail.windows(2).all(|pair| pair[0] != pair[1]));
    });
}

#[test]
fn only_the_front_state_lights_its_tile() {
    let center = grid_index(4, 4);
    let neighbor = grid_index(5, 4);
    let mut state = cyclic_deserialize(serde_json::json!({
        "states": 4, "threshold": 1, "range": 1, "cells": []
    }))
    .unwrap();
    state.cells[center] = 0;
    state.cells[neighbor] = 1;

    let first = cyclic_on_tick(state, &mut context());
    assert_eq!(first.cells[center], 1);
    assert_eq!(first.trigger_types[center], TileTriggerType::Activate);
    assert!(cyclic_render_model(&first).tiles[center]);

    let mut second_input = first;
    second_input.cells[neighbor] = 2;
    let second = cyclic_on_tick(second_input, &mut context());
    assert_eq!(second.cells[center], 2);
    assert_eq!(second.trigger_types[center], TileTriggerType::Deactivate);
    assert!(!cyclic_render_model(&second).tiles[center]);
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
    assert!(!state.trigger_types.contains(&TileTriggerType::Activate));
}
