use super::*;

fn ctx() -> BehaviorContext {
    BehaviorContext::new(120.0)
}
fn empty() -> PredatorPreyState {
    let mut s=predator_prey_init(serde_json::json!({"grassGrowChancePct":0,"herbivoreReproducePct":0,"predatorReproducePct":0,"starveTicks":3})).unwrap();
    s.cells.fill(EMPTY);
    s.energy.fill(0);
    s.trigger_types.fill(CellTriggerType::None);
    s
}

#[test]
fn menu_palette_normalize_and_serialization() {
    let m = predator_prey_config_menu();
    assert_eq!(m[0].key, "grassGrowChancePct");
    assert_eq!(m[0].max, Some(100));
    assert_eq!(m[3].key, "starveTicks");
    assert_eq!(m[3].max, Some(32));
    assert_eq!(m[4].key, "reseedEcosystem");
    let s=predator_prey_deserialize(serde_json::json!({"cells":[9,2],"energy":[99],"grassGrowChancePct":999,"herbivoreReproducePct":999,"predatorReproducePct":999,"starveTicks":99,"triggerTypes":["activate"]})).unwrap();
    assert_eq!(s.cells[0], PREDATOR);
    assert_eq!(s.energy[0], 32);
    assert_eq!(s.trigger_types[0], CellTriggerType::Stable);
    let v = predator_prey_serialize(&s).unwrap();
    assert!(v.get("triggerTypes").is_none());
    assert_eq!(
        predator_prey_serialize(&predator_prey_deserialize(v.clone()).unwrap()).unwrap(),
        v
    );
    let model = predator_prey_render_model(&s);
    assert_eq!(model.name, "predator prey");
    assert_eq!(model.palette.inactive, crate::palette::BLACK);
    assert_eq!(model.palette.stable, crate::palette::GREEN);
}

#[test]
fn empty_payload_serialization_stays_empty() {
    let state = predator_prey_deserialize(serde_json::json!({
        "cells": [], "energy": [], "grassGrowChancePct": 0
    }))
    .unwrap();
    assert!(state.cells.iter().all(|cell| *cell == EMPTY));
    let serialized = predator_prey_serialize(&state).unwrap();
    let restored = predator_prey_deserialize(serialized.clone()).unwrap();
    assert_eq!(predator_prey_serialize(&restored).unwrap(), serialized);
}

#[test]
fn grid_press_cycle_exact_world_cell() {
    let mut c = ctx();
    let s = empty();
    let s = predator_prey_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.cells[grid_index(2, 3)], GRASS);
    assert_eq!(s.trigger_types[grid_index(2, 3)], CellTriggerType::Stable);
    let s = predator_prey_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.cells[grid_index(2, 3)], HERBIVORE);
    assert_eq!(s.trigger_types[grid_index(2, 3)], CellTriggerType::Activate);
    let s = predator_prey_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.cells[grid_index(2, 3)], PREDATOR);
    let s = predator_prey_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.cells[grid_index(2, 3)], EMPTY);
    assert_eq!(
        s.trigger_types[grid_index(2, 3)],
        CellTriggerType::Deactivate
    );
}

#[test]
fn grass_regrowth_stable_and_herbivore_eats_without_burst() {
    let mut c = ctx();
    let mut s = empty();
    s.grass_grow_chance_pct = 100;
    let grown = predator_prey_on_tick(s, &mut c);
    assert!(grown
        .cells
        .iter()
        .enumerate()
        .any(|(i, cell)| *cell == GRASS && grown.trigger_types[i] == CellTriggerType::Stable));
    let mut s = empty();
    s.cells[grid_index(1, 1)] = HERBIVORE;
    s.energy[grid_index(1, 1)] = 3;
    s.cells[grid_index(1, 2)] = GRASS;
    let n = predator_prey_on_tick(s, &mut c);
    assert_eq!(n.cells[grid_index(1, 2)], HERBIVORE);
    assert_eq!(n.trigger_types[grid_index(1, 2)], CellTriggerType::Activate);
    assert_eq!(n.trigger_types[grid_index(2, 2)], CellTriggerType::None);
}

#[test]
fn predator_eat_burst_clips_and_does_not_mutate_or_repeat() {
    let mut c = ctx();
    let mut s = empty();
    s.cells[grid_index(0, 1)] = PREDATOR;
    s.energy[grid_index(0, 1)] = 3;
    s.cells[grid_index(0, 2)] = HERBIVORE;
    let n = predator_prey_on_tick(s, &mut c);
    assert_eq!(n.cells[grid_index(1, 2)], EMPTY);
    assert_eq!(n.trigger_types[grid_index(0, 2)], CellTriggerType::Activate);
    assert_eq!(n.trigger_types[grid_index(1, 2)], CellTriggerType::Activate);
    let n2 = predator_prey_on_tick(n, &mut c);
    assert_ne!(
        n2.trigger_types[grid_index(1, 2)],
        CellTriggerType::Activate
    );
}

#[test]
fn births_and_reseeded_animals_force_activate() {
    let mut c = ctx();
    let mut s = empty();
    s.herbivore_reproduce_pct = 100;
    s.cells[grid_index(1, 1)] = HERBIVORE;
    s.energy[grid_index(1, 1)] = 3;
    s.cells[grid_index(1, 2)] = GRASS;
    let n = predator_prey_on_tick(s, &mut c);
    assert_eq!(n.cells[grid_index(1, 1)], HERBIVORE);
    assert_eq!(n.trigger_types[grid_index(1, 1)], CellTriggerType::Activate);

    let mut s = empty();
    s.cells[0] = HERBIVORE;
    s.energy[0] = 3;
    s.cells[1] = PREDATOR;
    s.energy[1] = 3;
    let n = predator_prey_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "reseedEcosystem".into(),
        }),
        &mut c,
    );
    assert_eq!(n.trigger_types[0], CellTriggerType::Activate);
    assert_eq!(n.trigger_types[1], CellTriggerType::Activate);
}

#[test]
fn starvation_conflict_nonwrap_and_reseed() {
    let mut c = ctx();
    let mut s = empty();
    s.cells[grid_index(0, 0)] = HERBIVORE;
    s.energy[grid_index(0, 0)] = 1;
    s.cells[grid_index(7, 0)] = HERBIVORE;
    s.energy[grid_index(7, 0)] = 3;
    s.cells[grid_index(7, 7)] = PREDATOR;
    s.energy[grid_index(7, 7)] = 3;
    let n = predator_prey_on_tick(s, &mut c);
    assert_eq!(
        n.trigger_types[grid_index(0, 0)],
        CellTriggerType::Deactivate
    );
    let mut s = empty();
    s.cells[grid_index(1, 0)] = PREDATOR;
    s.energy[grid_index(1, 0)] = 3;
    s.cells[grid_index(0, 1)] = PREDATOR;
    s.energy[grid_index(0, 1)] = 3;
    s.cells[grid_index(0, 0)] = HERBIVORE;
    let n = predator_prey_on_tick(s, &mut c);
    assert!(n.cells.iter().filter(|x| **x == PREDATOR).count() >= 1);
    let mut s = empty();
    s.cells[0] = GRASS;
    let n = predator_prey_on_tick(s, &mut c);
    assert!(n.cells.contains(&HERBIVORE));
    assert!(n.cells.contains(&PREDATOR));
}

#[test]
fn full_grass_grid_reopens_without_firehose() {
    let mut c = ctx();
    let mut s = empty();
    s.cells.fill(GRASS);
    s.grass_grow_chance_pct = 100;

    let n = predator_prey_on_tick(s, &mut c);

    assert!(n.cells.contains(&EMPTY));
    assert!(
        n.trigger_types
            .iter()
            .filter(|trigger| **trigger == CellTriggerType::Deactivate)
            .count()
            <= 4
    );
}

#[test]
fn default_run_avoids_consecutive_empty_or_full_frames() {
    let mut c = ctx();
    let mut s = predator_prey_init(Value::Null).unwrap();
    let mut empty_run = 0;
    let mut full_run = 0;

    for _ in 0..300 {
        s = predator_prey_on_tick(s, &mut c);
        let model = predator_prey_render_model(&s);
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
