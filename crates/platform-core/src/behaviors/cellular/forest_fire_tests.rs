use super::*;

#[test]
fn spread_burnout_growth_input_action_and_render_contract() {
    let mut context = BehaviorContext::new(120.0);
    let mut state = forest_fire_init(serde_json::json!({
        "treeDensityPct": 0, "reseedThresholdPct": 0, "growChancePct": 0,
        "spreadChancePct": 100, "lightningChancePerThousand": 0
    }))
    .unwrap();
    state.cells[grid_index(1, 1)] = BURNING;
    state.cells[grid_index(2, 1)] = TREE;
    let next = forest_fire_on_tick(state, &mut context);
    assert_eq!(next.cells[grid_index(1, 1)], EMPTY);
    assert_eq!(next.cells[grid_index(2, 1)], BURNING);
    assert_eq!(
        next.trigger_types[grid_index(2, 1)],
        CellTriggerType::Activate
    );
    assert_eq!(
        next.trigger_types[grid_index(1, 1)],
        CellTriggerType::Deactivate
    );

    let grown = forest_fire_on_tick(
        forest_fire_init(serde_json::json!({
            "treeDensityPct": 0, "reseedThresholdPct": 0, "growChancePct": 100,
            "lightningChancePerThousand": 0
        }))
        .unwrap(),
        &mut context,
    );
    assert!(grown.cells.iter().all(|cell| *cell == TREE));
    assert!(grown
        .trigger_types
        .iter()
        .all(|trigger| *trigger == CellTriggerType::Stable));
    assert!(forest_fire_on_input(
        grown.clone(),
        DeviceInput::GridPress { x: 2, y: 3 },
        &mut context
    )
    .cells
    .contains(&BURNING));
    assert!(forest_fire_on_input(
        grown,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "igniteRandom".into()
        }),
        &mut context,
    )
    .cells
    .contains(&BURNING));

    let model = forest_fire_render_model(&next);
    assert_eq!(model.name, "forest fire");
    assert!(model.status_line.starts_with("T:"));
    assert_eq!(model.trigger_types.unwrap().len(), CELL_COUNT);
}

#[test]
fn normalize_config_and_deserialize_state() {
    let state = forest_fire_deserialize(serde_json::json!({
        "cells": [9], "triggerTypes": [], "treeDensityPct": 200,
        "growChancePct": 200, "spreadChancePct": 200, "reseedThresholdPct": 0,
        "lightningChancePerThousand": 200, "generation": 99, "tickCounter": 99
    }))
    .unwrap();
    assert_eq!(state.cells.len(), CELL_COUNT);
    assert_eq!(state.trigger_types.len(), CELL_COUNT);
    assert_eq!(state.cells[0], BURNING);
    assert_eq!(state.tree_density_pct, 100);
    assert_eq!(state.lightning_chance_per_thousand, 20);
    let serialized = crate::behaviors::native_impl::serialize(&state).unwrap();
    assert!(serialized.get("generation").is_none());
    assert!(serialized.get("tickCounter").is_none());
    assert!(serialized.get("triggerTypes").is_none());
}

#[test]
fn malformed_large_numbers_clamp_per_field() {
    let state = forest_fire_deserialize(serde_json::json!({
        "cells": [300, 1],
        "treeDensityPct": 300,
        "growChancePct": 101,
        "spreadChancePct": 999,
        "reseedThresholdPct": 0,
        "lightningChancePerThousand": 1000
    }))
    .unwrap();
    assert_eq!(state.cells[0], BURNING);
    assert_eq!(state.cells[1], TREE);
    assert_eq!(state.tree_density_pct, 100);
    assert_eq!(state.grow_chance_pct, 100);
    assert_eq!(state.spread_chance_pct, 100);
    assert_eq!(state.reseed_threshold_pct, 0);
    assert_eq!(state.lightning_chance_per_thousand, 20);
}

#[test]
fn malformed_transient_data_preserves_cells_and_clamps_negative_fields() {
    let state = forest_fire_deserialize(serde_json::json!({
        "cells": [1, 2],
        "triggerTypes": ["not-a-trigger"],
        "treeDensityPct": -1,
        "growChancePct": -2,
        "spreadChancePct": -3,
        "reseedThresholdPct": -4,
        "lightningChancePerThousand": -5
    }))
    .unwrap();
    assert_eq!(state.cells[0], TREE);
    assert_eq!(state.cells[1], BURNING);
    assert_eq!(state.tree_density_pct, 0);
    assert_eq!(state.grow_chance_pct, 0);
    assert_eq!(state.spread_chance_pct, 0);
    assert_eq!(state.reseed_threshold_pct, 0);
    assert_eq!(state.lightning_chance_per_thousand, 0);
    assert_eq!(state.trigger_types[0], CellTriggerType::Stable);
    assert_eq!(state.trigger_types[1], CellTriggerType::Activate);
}

#[test]
fn explicit_saved_cells_round_trip_without_reseed() {
    let state = forest_fire_deserialize(serde_json::json!({
        "cells": [1, 2],
        "treeDensityPct": 100,
        "growChancePct": 0,
        "spreadChancePct": 0,
        "reseedThresholdPct": 100,
        "lightningChancePerThousand": 0
    }))
    .unwrap();
    assert_eq!(state.cells.iter().filter(|cell| **cell != EMPTY).count(), 2);
    let serialized = crate::behaviors::native_impl::serialize(&state).unwrap();
    let restored = forest_fire_deserialize(serialized.clone()).unwrap();
    assert_eq!(
        crate::behaviors::native_impl::serialize(&restored).unwrap(),
        serialized
    );
}

#[test]
fn explicit_empty_init_stays_empty() {
    let state = forest_fire_init(serde_json::json!({
        "cells": [], "treeDensityPct": 100, "reseedThresholdPct": 100
    }))
    .unwrap();
    assert!(state.cells.iter().all(|cell| *cell == EMPTY));
}

#[test]
fn reseed_fills_only_empty_cells() {
    let mut state = ForestFireState {
        cells: vec![EMPTY; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        tree_density_pct: 100,
        grow_chance_pct: 0,
        spread_chance_pct: 0,
        reseed_threshold_pct: 100,
        lightning_chance_per_thousand: 0,
    };
    state.cells[0] = TREE;
    state.cells[CELL_COUNT - 1] = BURNING;

    reseed_if_needed(&mut state);

    assert_eq!(state.cells[0], TREE);
    assert_eq!(state.cells[CELL_COUNT - 1], BURNING);
    assert!(state.cells.iter().all(|cell| *cell != EMPTY));
}

#[test]
fn input_replaces_stale_triggers_and_uses_exact_world_coordinate() {
    let mut context = BehaviorContext::new(120.0);
    let mut state = forest_fire_init(serde_json::json!({
        "treeDensityPct": 0, "reseedThresholdPct": 0, "growChancePct": 0,
        "cells": [1], "triggerTypes": ["deactivate", "activate"]
    }))
    .unwrap();
    state.cells[grid_index(2, 3)] = TREE;
    let state = forest_fire_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_eq!(state.cells[grid_index(2, 3)], BURNING);
    assert_eq!(
        state.trigger_types[grid_index(2, 3)],
        CellTriggerType::Activate
    );
    assert_eq!(
        state.trigger_types[grid_index(0, 0)],
        CellTriggerType::Stable
    );
    assert!(state
        .trigger_types
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != grid_index(0, 0) && *index != grid_index(2, 3))
        .all(|(_, trigger)| *trigger == CellTriggerType::None));
}

#[test]
fn pressing_already_burning_cell_does_not_reactivate() {
    let mut context = BehaviorContext::new(120.0);
    let mut state = forest_fire_init(serde_json::json!({
        "treeDensityPct": 0, "reseedThresholdPct": 0, "growChancePct": 0
    }))
    .unwrap();
    state.cells[grid_index(2, 3)] = BURNING;
    state.trigger_types = vec![CellTriggerType::None; CELL_COUNT];

    let state = forest_fire_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);

    assert_eq!(state.cells[grid_index(2, 3)], BURNING);
    assert_eq!(state.trigger_types[grid_index(2, 3)], CellTriggerType::None);
    assert!(state
        .trigger_types
        .iter()
        .all(|trigger| *trigger == CellTriggerType::None));
}

#[test]
fn edge_spread_does_not_wrap() {
    let mut context = BehaviorContext::new(120.0);
    let mut state = forest_fire_init(serde_json::json!({
        "treeDensityPct": 0, "reseedThresholdPct": 0, "growChancePct": 0,
        "spreadChancePct": 100, "lightningChancePerThousand": 0
    }))
    .unwrap();
    state.cells[grid_index(0, 0)] = BURNING;
    state.cells[grid_index(GRID_WIDTH - 1, 0)] = TREE;
    let next = forest_fire_on_tick(state, &mut context);
    assert_eq!(next.cells[grid_index(GRID_WIDTH - 1, 0)], TREE);
    assert_eq!(
        next.trigger_types[grid_index(GRID_WIDTH - 1, 0)],
        CellTriggerType::Stable
    );
}

#[test]
fn reseed_normalizes_render_and_trigger_semantics() {
    let state = forest_fire_init(serde_json::json!({
        "treeDensityPct": 25, "reseedThresholdPct": 10, "growChancePct": 0,
        "triggerTypes": ["activate", "deactivate"]
    }))
    .unwrap();
    let model = forest_fire_render_model(&state);
    let triggers = model.trigger_types.unwrap();
    for (index, cell) in state.cells.iter().enumerate() {
        assert_eq!(model.cells[index], *cell != EMPTY);
        assert_eq!(
            triggers[index],
            match *cell {
                TREE => CellTriggerType::Stable,
                BURNING => CellTriggerType::Activate,
                _ => CellTriggerType::None,
            }
        );
    }
}

#[test]
fn reseed_repopulates_near_empty_forest() {
    let state = forest_fire_init(serde_json::json!({
        "treeDensityPct": 25, "reseedThresholdPct": 10, "growChancePct": 0
    }))
    .unwrap();
    assert!(state.cells.iter().filter(|cell| **cell == TREE).count() >= CELL_COUNT / 4);
    assert!(state
        .cells
        .iter()
        .zip(state.trigger_types.iter())
        .all(|(cell, trigger)| if *cell == TREE {
            *trigger == CellTriggerType::Stable
        } else {
            *trigger == CellTriggerType::None
        }));
}

#[test]
fn config_menu_matches_contract() {
    let menu = forest_fire_config_menu();
    assert_eq!(menu[0].key, "treeDensityPct");
    assert_eq!(menu[4].key, "lightningChancePerThousand");
    assert_eq!(menu[4].label, "Lightning");
    assert_eq!(menu[4].min, Some(0));
    assert_eq!(menu[4].max, Some(20));
    assert_eq!(menu[5].key, "igniteRandom");
}
