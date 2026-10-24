use super::*;

#[test]
fn state_machine_cycles_firing_refractory_dead_with_threshold() {
    let mut state =
        brain_init(serde_json::json!({ "fireThreshold": 1, "randomSeedCells": 0 })).unwrap();
    state.cells[grid_index(1, 1)] = 1;
    let mut context = BehaviorContext::new(120.0);

    let next = brain_on_tick(state, &mut context);
    assert_eq!(next.cells[grid_index(0, 0)], 1);
    let next = brain_on_tick(next, &mut context);
    assert_eq!(next.cells[grid_index(0, 0)], 2);
    let next = brain_on_tick(next, &mut context);
    assert_eq!(next.cells[grid_index(0, 0)], 0);
}

#[test]
fn fire_threshold_prevents_insufficient_neighbors() {
    let mut state =
        brain_init(serde_json::json!({ "fireThreshold": 3, "randomSeedCells": 0 })).unwrap();
    state.cells[grid_index(1, 1)] = 1;
    state.cells[grid_index(0, 1)] = 1;

    let next = brain_on_tick(state, &mut BehaviorContext::new(120.0));
    assert_eq!(next.cells[grid_index(0, 0)], 0);
}

#[test]
fn render_input_and_config_menu_match_legacy_contract() {
    let state = brain_init(Value::Null).unwrap();
    let mut context = BehaviorContext::new(120.0);
    let toggled = brain_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_eq!(toggled.cells[grid_index(2, 3)], 1);
    assert_eq!(
        toggled.trigger_types[grid_index(2, 3)],
        CellTriggerType::Activate
    );
    let toggled = brain_on_input(toggled, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_eq!(toggled.cells[grid_index(2, 3)], 0);
    assert_eq!(
        toggled.trigger_types[grid_index(2, 3)],
        CellTriggerType::Deactivate
    );

    let mut state =
        brain_init(serde_json::json!({ "fireThreshold": 1, "randomSeedCells": 0 })).unwrap();
    state.cells[grid_index(5, 5)] = 1;
    let next = brain_on_tick(state, &mut context);
    let model = brain_render_model(&next);
    assert_eq!(model.cells.len(), CELL_COUNT);
    assert!(model.cells.iter().any(|cell| *cell));
    assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);

    let menu = brain_config_menu();
    assert_eq!(menu.len(), 5);
    assert_eq!(menu[0].key, "fireThreshold");
    assert_eq!(menu[1].key, "seedInterval");
    assert_eq!(menu[2].key, "spawnStep");
    assert_eq!(menu[3].key, "randomSeedCells");
    assert_eq!(menu[4].key, "seedRandom");
}

#[test]
fn default_run_avoids_consecutive_empty_or_full_frames() {
    let mut state = brain_init(Value::Null).unwrap();
    let mut context = BehaviorContext::new(120.0);
    let mut empty_run = 0;
    let mut full_run = 0;

    for _ in 0..300 {
        state = brain_on_tick(state, &mut context);
        let model = brain_render_model(&state);
        empty_run = if model.cells.iter().all(|cell| !*cell) {
            empty_run + 1
        } else {
            0
        };
        full_run = if model.cells.iter().all(|cell| *cell) {
            full_run + 1
        } else {
            0
        };
        assert!(empty_run <= 1);
        assert!(full_run <= 1);
    }
}

#[test]
fn malformed_saved_state_normalizes_vectors_parameters_and_counters() {
    let state = brain_deserialize(serde_json::json!({
        "cells": [1, 2, 9, "bad"],
        "triggerTypes": ["activate", "bad"],
        "fireThreshold": -1,
        "randomSeedCells": -1,
        "seedInterval": 999,
        "spawnStep": 999,
        "generation": u64::MAX,
        "tickCounter": u64::MAX
    }))
    .unwrap();

    assert_eq!(state.cells.len(), CELL_COUNT);
    assert_eq!(state.cells[..4], [1, 2, 2, 0]);
    assert_eq!(state.trigger_types.len(), CELL_COUNT);
    assert_eq!(state.trigger_types[0], CellTriggerType::Activate);
    assert_eq!(state.trigger_types[1], CellTriggerType::None);
    assert_eq!(state.fire_threshold, 1);
    assert_eq!(state.random_seed_cells, 0);
    assert_eq!(state.seed_interval, 30);
    assert_eq!(state.spawn_step, 63);
    assert_eq!(state.generation, 0);
    assert_eq!(state.tick_counter, 0);

    let next = brain_on_tick(state, &mut BehaviorContext::new(120.0));
    assert_eq!(next.cells.len(), CELL_COUNT);
    assert_eq!(next.trigger_types.len(), CELL_COUNT);
}

#[test]
fn manual_and_scheduled_seeding_use_exact_configured_counts() {
    let mut context = BehaviorContext::new(120.0);
    let state = brain_init(serde_json::json!({
        "cells": [],
        "randomSeedCells": 4,
        "seedInterval": 0
    }))
    .unwrap();
    let seeded = brain_on_input(
        state,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedRandom".into(),
        }),
        &mut context,
    );
    assert_eq!(seeded.cells.iter().filter(|cell| **cell == 1).count(), 4);

    let zero = brain_init(serde_json::json!({
        "cells": [],
        "randomSeedCells": 0
    }))
    .unwrap();
    let zero = brain_on_input(
        zero.clone(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedRandom".into(),
        }),
        &mut context,
    );
    assert_eq!(
        zero,
        brain_init(serde_json::json!({
            "cells": [],
            "randomSeedCells": 0
        }))
        .unwrap()
    );

    let scheduled = brain_init(serde_json::json!({
        "cells": [],
        "randomSeedCells": 20,
        "seedInterval": 1,
        "spawnStep": 0
    }))
    .unwrap();
    let scheduled = brain_on_tick(scheduled, &mut context);
    assert_eq!(
        scheduled.cells.iter().filter(|cell| **cell == 1).count(),
        20
    );
}
