use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

fn empty_state(symmetry: &str) -> CrystalGrowthState {
    let mut state = crystal_growth_init(serde_json::json!({
        "growthChancePct": 100,
        "seedInterval": 0,
        "cellLife": 0,
        "symmetry": symmetry
    }))
    .unwrap();
    state.cells = vec![EMPTY; CELL_COUNT];
    state.ages = vec![0; CELL_COUNT];
    state.trigger_types = vec![CellTriggerType::None; CELL_COUNT];
    state
}

#[test]
fn init_menu_and_palette_contract() {
    let state = crystal_growth_init(serde_json::json!({})).unwrap();
    assert_eq!(state.cells.len(), CELL_COUNT);
    assert_eq!(state.ages.len(), CELL_COUNT);
    assert_eq!(state.trigger_types.len(), CELL_COUNT);
    assert!(state.cells.iter().any(|cell| *cell != EMPTY));

    let menu = crystal_growth_config_menu();
    assert_eq!(menu[0].key, "growthChancePct");
    assert_eq!(menu[0].min, Some(0));
    assert_eq!(menu[0].max, Some(100));
    assert_eq!(menu[1].key, "seedInterval");
    assert_eq!(menu[1].max, Some(64));
    assert_eq!(menu[2].key, "seedStep");
    assert_eq!(menu[2].max, Some(63));
    assert_eq!(menu[3].key, "cellLife");
    assert_eq!(menu[3].max, Some(256));
    assert_eq!(menu[4].key, "symmetry");
    assert_eq!(
        menu[4].options.as_ref().unwrap(),
        &vec!["cross", "diagonal", "snowflake"]
    );
    assert_eq!(menu[5].key, "seedCrystal");

    let model = crystal_growth_render_model(&state);
    assert_eq!(model.name, "crystal growth");
    assert!(model.status_line.starts_with("Cr:"));
    assert_eq!(model.palette.active, [220, 255, 255]);
    assert_eq!(model.palette.stable, [40, 180, 255]);
}

#[test]
fn grid_press_exact_cell_empty_activate_existing_stable() {
    let mut context = context();
    let state = empty_state("cross");
    let state = crystal_growth_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_ne!(state.cells[grid_index(2, 3)], EMPTY);
    assert_eq!(
        state.trigger_types[grid_index(2, 3)],
        CellTriggerType::Activate
    );

    let state = crystal_growth_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_ne!(state.cells[grid_index(2, 3)], EMPTY);
    assert_eq!(state.ages[grid_index(2, 3)], 0);
    assert_eq!(
        state.trigger_types[grid_index(2, 3)],
        CellTriggerType::Stable
    );
}

#[test]
fn action_seed_handles_empty_and_full_grid() {
    let mut context = context();
    let state = empty_state("cross");
    let state = crystal_growth_on_input(
        state,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedCrystal".into(),
        }),
        &mut context,
    );
    assert_eq!(state.cells.iter().filter(|cell| **cell != EMPTY).count(), 1);
    assert_eq!(
        state
            .trigger_types
            .iter()
            .filter(|trigger| **trigger == CellTriggerType::Activate)
            .count(),
        1
    );

    let mut full = state;
    full.cells = vec![1; CELL_COUNT];
    full.ages = vec![7; CELL_COUNT];
    let full = crystal_growth_on_input(
        full,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedCrystal".into(),
        }),
        &mut context,
    );
    assert!(full.cells.iter().all(|cell| *cell != EMPTY));
    assert!(full
        .trigger_types
        .iter()
        .all(|trigger| *trigger == CellTriggerType::Stable));
    assert!(full.ages.contains(&0));
}

#[test]
fn deterministic_growth_matches_symmetry_without_cascade_or_wrap() {
    let mut context = context();
    let mut cross = empty_state("cross");
    cross.cells[grid_index(0, 0)] = 1;
    let cross = crystal_growth_on_tick(cross, &mut context);
    assert_ne!(cross.cells[grid_index(1, 0)], EMPTY);
    assert_ne!(cross.cells[grid_index(0, 1)], EMPTY);
    assert_eq!(cross.cells[grid_index(1, 1)], EMPTY);
    assert_eq!(
        cross.trigger_types[grid_index(1, 0)],
        CellTriggerType::Activate
    );

    let mut diagonal = empty_state("diagonal");
    diagonal.cells[grid_index(1, 1)] = 1;
    let diagonal = crystal_growth_on_tick(diagonal, &mut context);
    assert_ne!(diagonal.cells[grid_index(0, 0)], EMPTY);
    assert_ne!(diagonal.cells[grid_index(2, 2)], EMPTY);
    assert_eq!(diagonal.cells[grid_index(1, 0)], EMPTY);

    let mut snow = empty_state("snowflake");
    snow.cells[grid_index(1, 2)] = 1;
    let snow = crystal_growth_on_tick(snow, &mut context);
    assert_ne!(snow.cells[grid_index(2, 3)], EMPTY);
    assert_ne!(snow.cells[grid_index(0, 1)], EMPTY);
    assert_eq!(snow.cells[grid_index(2, 1)], EMPTY);
}

#[test]
fn cell_life_dissolves_and_empty_grid_reseeds() {
    let mut context = context();
    let mut state = empty_state("cross");
    state.cell_life = 1;
    state.growth_chance_pct = 0;
    state.cells[grid_index(2, 2)] = 1;
    let state = crystal_growth_on_tick(state, &mut context);
    assert!(state.cells.iter().any(|cell| *cell != EMPTY));
    assert_eq!(
        state.trigger_types[grid_index(2, 2)],
        CellTriggerType::Deactivate
    );
    assert_eq!(
        state
            .trigger_types
            .iter()
            .filter(|trigger| **trigger == CellTriggerType::Activate)
            .count(),
        1
    );
}

#[test]
fn malformed_short_state_normalizes_and_serializes() {
    let state = crystal_growth_deserialize(serde_json::json!({
        "cells": [9, 2],
        "ages": [999999],
        "triggerTypes": [],
        "growthChancePct": 999,
        "seedInterval": 999,
        "seedStep": 999,
        "cellLife": 999,
        "symmetry": "bad",
        "tickCounter": 42
    }))
    .unwrap();
    assert_eq!(state.cells[0], MAX_PHASE);
    assert_eq!(state.cells.len(), CELL_COUNT);
    assert_eq!(state.ages.len(), CELL_COUNT);
    assert_eq!(state.trigger_types.len(), CELL_COUNT);
    assert_eq!(state.growth_chance_pct, 100);
    assert_eq!(state.seed_interval, 64);
    assert_eq!(state.seed_step, 63);
    assert_eq!(state.cell_life, 256);
    assert_eq!(state.symmetry, "cross");
    let serialized = crystal_growth_serialize(&state).unwrap();
    assert!(serialized.get("tickCounter").is_none());
    assert!(serialized.get("triggerTypes").is_none());
    assert_eq!(serialized.get("symmetry").unwrap(), "cross");
}

#[test]
fn restored_crystals_are_stable_and_serialization_is_stable() {
    let mut state = empty_state("cross");
    state.cells[grid_index(1, 1)] = 2;
    state.ages[grid_index(1, 1)] = 7;
    state.trigger_types[grid_index(1, 1)] = CellTriggerType::Activate;
    state.tick_counter = 9;

    let serialized = crystal_growth_serialize(&state).unwrap();
    assert!(serialized.get("triggerTypes").is_none());
    assert!(serialized.get("tickCounter").is_none());

    let restored = crystal_growth_deserialize(serialized.clone()).unwrap();
    assert_eq!(
        restored.trigger_types[grid_index(1, 1)],
        CellTriggerType::Stable
    );
    assert!(!restored.trigger_types.contains(&CellTriggerType::Activate));
    assert_eq!(crystal_growth_serialize(&restored).unwrap(), serialized);
}

#[test]
fn default_tail_stays_bounded_and_non_terminal() {
    let mut context = context();
    let mut state = crystal_growth_init(serde_json::json!({})).unwrap();
    let mut same = 0;
    let mut terminal = 0;
    let mut previous = visible(&state.cells);
    for _ in 0..300 {
        state = crystal_growth_on_tick(state, &mut context);
        let next = visible(&state.cells);
        same = if next == previous { same + 1 } else { 0 };
        terminal = if next.iter().all(|cell| *cell) || next.iter().all(|cell| !*cell) {
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
                    CellTriggerType::Activate | CellTriggerType::Deactivate
                )
            })
            .count();
        assert!(bursts <= 24);
        previous = next;
    }
}
