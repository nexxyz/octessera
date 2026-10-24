use super::*;

#[test]
fn empty_config_seeds_deterministic_ant_and_autospawns_are_bounded() {
    let mut context = BehaviorContext::new(120.0);
    let mut a = ant_init(serde_json::json!({})).unwrap();
    let mut b = ant_init(serde_json::json!({})).unwrap();
    assert_eq!(a.ants, b.ants);
    assert_eq!(a.ants.len(), 1);
    let mut max_activates = 0;
    for _ in 0..32 {
        a = ant_on_tick(a, &mut context);
        b = ant_on_tick(b, &mut context);
        assert_eq!(a.ants, b.ants);
        max_activates = max_activates.max(
            a.trigger_types
                .iter()
                .filter(|trigger| **trigger == CellTriggerType::Activate)
                .count(),
        );
    }
    assert!(max_activates <= 8);
}

#[test]
fn ant_moves_flips_wraps_and_respects_max_ants() {
    let mut context = BehaviorContext::new(120.0);
    let mut state = ant_init(serde_json::json!({ "maxAnts": 1, "ants": [] })).unwrap();
    state = ant_on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut context);
    let limited = ant_on_input(
        state.clone(),
        DeviceInput::GridPress { x: 2, y: 2 },
        &mut context,
    );
    assert_eq!(limited.ants.len(), 1);

    let ticked = ant_on_tick(state, &mut context);
    assert!(ticked.cells[grid_index(0, 0)]);
    assert_eq!(ticked.ants[0].x, 1);
    assert_eq!(ticked.ants[0].y, 0);
    assert_eq!(ticked.ants[0].dir, 1);

    let ticked = ant_on_tick(
        AntState {
            ants: vec![AntAgent { x: 0, y: 0, dir: 0 }],
            cells: {
                let mut cells = vec![false; CELL_COUNT];
                cells[grid_index(0, 0)] = true;
                cells
            },
            trigger_types: vec![CellTriggerType::None; CELL_COUNT],
            max_ants: 1,
            auto_spawn_interval: 0,
            spawn_step: 0,
            tick_counter: 0,
        },
        &mut context,
    );
    assert_eq!(ticked.ants[0].x, GRID_WIDTH - 1);
    assert_eq!(ticked.ants[0].y, 0);
}

#[test]
fn ant_render_and_config_menu_match_contract() {
    let mut state = ant_init(serde_json::json!({ "ants": [] })).unwrap();
    state.ants.push(AntAgent { x: 1, y: 2, dir: 0 });
    let model = ant_render_model(&state);
    assert_eq!(model.name, "ant");
    assert_eq!(model.status_line, "1 ant");
    assert!(model.cells[grid_index(1, 2)]);
    let menu = ant_config_menu();
    assert_eq!(
        menu.iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec!["maxAnts", "autoSpawnInterval", "spawnStep", "spawnAnt"]
    );
}

#[test]
fn malformed_saved_state_normalizes_vectors_agents_parameters_and_counters() {
    let state = ant_deserialize(serde_json::json!({
        "ants": [
            { "x": 999, "y": 1000, "dir": 9 },
            { "x": 2, "y": 3, "dir": 2 },
            { "x": 4, "y": 5, "dir": 1 }
        ],
        "cells": [true, "bad"],
        "triggerTypes": ["activate", "bad"],
        "maxAnts": 2,
        "autoSpawnInterval": -1,
        "spawnStep": 999,
        "tickCounter": u64::MAX
    }))
    .unwrap();

    assert_eq!(state.ants.len(), 2);
    assert_eq!(state.ants[0], AntAgent { x: 7, y: 0, dir: 1 });
    assert_eq!(state.ants[1], AntAgent { x: 2, y: 3, dir: 2 });
    assert_eq!(state.cells.len(), CELL_COUNT);
    assert!(state.cells[0]);
    assert_eq!(state.trigger_types.len(), CELL_COUNT);
    assert_eq!(state.trigger_types[0], CellTriggerType::Activate);
    assert_eq!(state.trigger_types[1], CellTriggerType::None);
    assert_eq!(state.max_ants, 2);
    assert_eq!(state.auto_spawn_interval, 0);
    assert_eq!(state.spawn_step, 63);
    assert_eq!(state.tick_counter, 0);

    let next = ant_on_tick(state, &mut BehaviorContext::new(120.0));
    assert_eq!(next.cells.len(), CELL_COUNT);
    assert_eq!(next.trigger_types.len(), CELL_COUNT);
}

#[test]
fn missing_saved_state_seeds_default_but_explicit_empty_stays_empty() {
    assert_eq!(
        ant_deserialize(serde_json::json!({})).unwrap().ants.len(),
        1
    );
    assert!(ant_deserialize(serde_json::json!({ "ants": [] }))
        .unwrap()
        .ants
        .is_empty());
}

#[test]
fn scheduled_spawning_stops_at_capacity_and_zero_disables_it() {
    let mut context = BehaviorContext::new(120.0);
    let mut state = ant_init(serde_json::json!({
        "ants": [],
        "maxAnts": 3,
        "autoSpawnInterval": 1,
        "spawnStep": 0
    }))
    .unwrap();
    for _ in 0..8 {
        state = ant_on_tick(state, &mut context);
    }
    assert_eq!(state.ants.len(), 3);

    let state = ant_init(serde_json::json!({
        "ants": [],
        "maxAnts": 3,
        "autoSpawnInterval": 0
    }))
    .unwrap();
    let state = ant_on_tick(state, &mut context);
    assert!(state.ants.is_empty());
}
