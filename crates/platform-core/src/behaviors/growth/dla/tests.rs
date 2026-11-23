use super::*;

#[test]
fn init_reads_spawn_step_from_config() {
    let state = dla_init(serde_json::json!({ "spawnStep": 7 })).unwrap();
    assert_eq!(state.spawn_step, 7);
    let state = dla_init(serde_json::json!({ "spawnStep": 999 })).unwrap();
    assert_eq!(state.spawn_step, 63);
}

#[test]
fn init_creates_seed_cluster_and_grid_press_toggles_the_tiles_cell() {
    let mut context = BehaviorContext::new(120.0);
    let state = dla_init(Value::Null).unwrap();
    let cx = GRID_WIDTH / 2;
    let cy = GRID_HEIGHT / 2;
    assert!(state.cells[grid_index(cx, cy)]);
    assert!(state.cells[grid_index(cx + 1, cy)]);
    assert!(state.cells[grid_index(cx, cy + 1)]);

    let toggled = dla_on_input(state, DeviceInput::GridPress { x: 1, y: 1 }, &mut context);
    assert!(toggled.cells[grid_index(1, 1)]);
    assert_eq!(
        toggled.trigger_types[grid_index(1, 1)],
        TileTriggerType::Activate
    );
    let toggled = dla_on_input(toggled, DeviceInput::GridPress { x: 1, y: 1 }, &mut context);
    assert!(!toggled.cells[grid_index(1, 1)]);
    assert_eq!(
        toggled.trigger_types[grid_index(1, 1)],
        TileTriggerType::Deactivate
    );
}

#[test]
fn dla_render_and_config_menu_match_contract() {
    let state = dla_init(Value::Null).unwrap();
    let model = dla_render_model(&state);
    assert_eq!(model.name, "dla");
    assert_eq!(model.trigger_types.as_ref().unwrap().len(), TILE_COUNT);
    assert_eq!(
        dla_config_menu()
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec!["spawnInterval", "spawnStep", "cellLife", "seedCluster"]
    );
}

#[test]
fn old_payloads_normalize_cells_and_cell_life() {
    let mut context = BehaviorContext::new(120.0);
    let state = DlaState {
        cells: vec![true, false],
        trigger_types: Vec::new(),
        spawn_interval: 0,
        spawn_step: 0,
        cell_life: 300,
        tick_counter: 0,
    };

    let ticked = dla_on_tick(state, &mut context);

    assert_eq!(ticked.cells.len(), TILE_COUNT);
    assert_eq!(ticked.cell_life, MAX_CELL_LIFE);
}

#[test]
fn old_short_payload_grid_input_normalizes_before_trigger_recompute() {
    let mut context = BehaviorContext::new(120.0);
    let state = DlaState {
        cells: vec![true],
        trigger_types: Vec::new(),
        spawn_interval: 0,
        spawn_step: 0,
        cell_life: 0,
        tick_counter: 0,
    };

    let next = dla_on_input(state, DeviceInput::GridPress { x: 1, y: 0 }, &mut context);

    assert_eq!(next.cells.len(), TILE_COUNT);
    assert_eq!(next.trigger_types.len(), TILE_COUNT);
    assert_eq!(
        next.trigger_types[grid_index(1, 0)],
        TileTriggerType::Activate
    );
}

#[test]
fn seed_cluster_action_and_no_frontier_tick_are_stable() {
    crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
        let mut context = BehaviorContext::new(120.0);
        let empty = DlaState {
            cells: vec![false; TILE_COUNT],
            trigger_types: vec![TileTriggerType::None; TILE_COUNT],
            spawn_interval: 1,
            spawn_step: 0,
            cell_life: 0,
            tick_counter: 0,
        };
        let ticked = dla_on_tick(empty.clone(), &mut context);
        assert!(ticked.cells.iter().all(|cell| !*cell));
        assert!(ticked
            .trigger_types
            .iter()
            .all(|trigger| *trigger == TileTriggerType::None));

        let seeded = dla_on_input(
            empty,
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "seedCluster".into(),
            }),
            &mut context,
        );
        assert!(seeded.cells.iter().filter(|cell| **cell).count() >= 1);
        assert!(seeded.trigger_types.contains(&TileTriggerType::Activate));
    });
}

#[test]
fn cell_life_one_breaks_every_cell_off_and_reseeds_when_empty() {
    let mut context = BehaviorContext::new(120.0);
    let state = DlaState {
        cells: vec![true; TILE_COUNT],
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        spawn_interval: 0,
        spawn_step: 0,
        cell_life: 1,
        tick_counter: 0,
    };

    let ticked = dla_on_tick(state, &mut context);

    assert_eq!(ticked.cells.iter().filter(|cell| **cell).count(), 3);
}

#[test]
fn a_full_cluster_keeps_renewing_at_a_rate_set_by_cell_life() {
    let churn = |cell_life: usize| {
        crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
            let mut context = BehaviorContext::new(120.0);
            let mut state = dla_init(serde_json::json!({ "cellLife": cell_life })).unwrap();
            let mut changes = 0;
            for tick in 0..400 {
                let previous = state.cells.clone();
                state = dla_on_tick(state, &mut context);
                if tick >= 200 {
                    changes += previous
                        .iter()
                        .zip(&state.cells)
                        .filter(|(a, b)| a != b)
                        .count();
                }
            }
            changes
        })
    };
    let (short, long) = (churn(128), churn(256));
    assert!(long > 20, "a long-lived cluster still renews: {long}");
    assert!(short > long * 3 / 2, "{short} vs {long}");
}

#[test]
fn cell_life_zero_disables_break_off() {
    let mut context = BehaviorContext::new(120.0);
    let state = DlaState {
        cells: vec![true; TILE_COUNT],
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        spawn_interval: 0,
        spawn_step: 0,
        cell_life: 0,
        tick_counter: 0,
    };

    let ticked = dla_on_tick(state, &mut context);

    assert_eq!(
        ticked.cells.iter().filter(|cell| **cell).count(),
        TILE_COUNT
    );
}

#[test]
fn default_tail_stays_bounded_and_non_terminal() {
    crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
        let mut context = BehaviorContext::new(120.0);
        let mut state = dla_init(Value::Null).unwrap();
        let mut same = 0;
        let mut terminal = 0;
        let mut previous = state.cells.clone();
        for _ in 0..300 {
            state = dla_on_tick(state, &mut context);
            same = if state.cells == previous { same + 1 } else { 0 };
            terminal =
                if state.cells.iter().all(|cell| *cell) || state.cells.iter().all(|cell| !*cell) {
                    terminal + 1
                } else {
                    0
                };
            assert!(same <= 2);
            assert!(terminal <= 2);
            let bursts = state
                .trigger_types
                .iter()
                .filter(|trigger| {
                    matches!(
                        trigger,
                        TileTriggerType::Activate | TileTriggerType::Deactivate
                    )
                })
                .count();
            assert!(bursts <= 4);
            previous = state.cells.clone();
        }
    });
}

fn grown(seed: u64, config: Value, ticks: usize) -> Vec<DlaState> {
    let mut random = crate::behavior_random::LayerRandom::new(Some(seed));
    random.scope(|| {
        let mut context = BehaviorContext::new(120.0);
        let mut state = dla_init(config).unwrap();
        (0..ticks)
            .map(|_| {
                state = dla_on_tick(state.clone(), &mut context);
                state.clone()
            })
            .collect()
    })
}

#[test]
fn walkers_grow_a_different_cluster_in_every_random_stream() {
    let config = serde_json::json!({ "cellLife": 0 });
    let first = grown(1, config.clone(), 30);
    assert_ne!(first[29].cells, grown(2, config.clone(), 30)[29].cells);
    assert_eq!(first[29].cells, grown(1, config, 30)[29].cells);
}

#[test]
fn without_cell_life_the_cluster_starts_over_once_it_reaches_the_rim() {
    let states = grown(
        3,
        serde_json::json!({ "cellLife": 0, "spawnInterval": 1 }),
        200,
    );
    let restart = states
        .windows(2)
        .position(|pair| {
            touches_rim(&pair[0].cells) && pair[1].cells.iter().filter(|cell| **cell).count() == 3
        })
        .expect("the cluster starts over");
    assert!(states[..=restart]
        .iter()
        .all(|state| state.cells.iter().any(|cell| !*cell)));
}

#[test]
fn a_saved_cluster_with_cell_ages_still_loads_and_saves_without_them() {
    let behavior = crate::behaviors::get_native_behavior("dla").unwrap();
    let mut cells = vec![false; TILE_COUNT];
    cells[grid_index(2, 3)] = true;
    let saved = serde_json::json!({
        "cells": cells,
        "ages": vec![40; TILE_COUNT],
        "triggerTypes": [],
        "spawnInterval": 2,
        "spawnStep": 0,
        "cellLife": 96
    });
    let state = behavior.deserialize(saved).unwrap();
    let resaved = behavior.serialize(&state).unwrap();
    assert!(resaved.get("ages").is_none());
    assert_eq!(resaved["cells"][grid_index(2, 3)], true);
    assert_eq!(resaved["cellLife"], 96);
}
