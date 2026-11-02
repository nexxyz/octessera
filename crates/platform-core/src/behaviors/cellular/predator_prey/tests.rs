use super::*;

fn ctx() -> BehaviorContext {
    BehaviorContext::new(120.0)
}
fn empty() -> PredatorPreyState {
    let mut s=predator_prey_init(serde_json::json!({"grassGrowChancePct":0,"herbivoreReproducePct":0,"predatorReproducePct":0,"starveTicks":3})).unwrap();
    s.tiles.fill(EMPTY);
    s.energy.fill(0);
    s.trigger_types.fill(TileTriggerType::None);
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
    let s=predator_prey_deserialize(serde_json::json!({"tiles":[9,2],"energy":[99],"grassGrowChancePct":999,"herbivoreReproducePct":999,"predatorReproducePct":999,"starveTicks":99,"triggerTypes":["activate"]})).unwrap();
    assert_eq!(s.tiles[0], PREDATOR);
    assert_eq!(s.energy[0], 32);
    assert_eq!(s.trigger_types[0], TileTriggerType::Stable);
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
fn malformed_saved_field_preserves_valid_persistent_fields() {
    let state = predator_prey_deserialize(serde_json::json!({
        "tiles": [HERBIVORE, GRASS],
        "energy": [7, 4],
        "grassGrowChancePct": 0,
        "herbivoreReproducePct": 23,
        "predatorReproducePct": 9,
        "starveTicks": 4,
        "triggerTypes": ["activate", "not-a-trigger"]
    }))
    .unwrap();

    assert_eq!(state.tiles[0], HERBIVORE);
    assert_eq!(state.tiles[1], GRASS);
    assert_eq!(state.energy[0], 4);
    assert_eq!(state.energy[1], 0);
    assert_eq!(state.grass_grow_chance_pct, 0);
    assert_eq!(state.herbivore_reproduce_pct, 23);
    assert_eq!(state.predator_reproduce_pct, 9);
    assert_eq!(state.starve_ticks, 4);
}

#[test]
fn explicit_empty_init_state_stays_empty() {
    let state = predator_prey_init(serde_json::json!({
        "tiles": [],
        "grassGrowChancePct": 0
    }))
    .unwrap();

    assert!(state.tiles.iter().all(|tile| *tile == EMPTY));
}

#[test]
fn missing_saved_state_seeds_but_explicit_empty_saved_state_stays_empty() {
    let seeded = predator_prey_deserialize(serde_json::json!({})).unwrap();
    assert!(seeded.tiles.contains(&HERBIVORE));
    assert!(seeded.tiles.contains(&PREDATOR));

    let empty = predator_prey_deserialize(serde_json::json!({"tiles": []})).unwrap();
    assert!(empty.tiles.iter().all(|tile| *tile == EMPTY));
}

#[test]
fn empty_payload_serialization_stays_empty() {
    let state = predator_prey_deserialize(serde_json::json!({
        "tiles": [], "energy": [], "grassGrowChancePct": 0
    }))
    .unwrap();
    assert!(state.tiles.iter().all(|tile| *tile == EMPTY));
    let serialized = predator_prey_serialize(&state).unwrap();
    let restored = predator_prey_deserialize(serialized.clone()).unwrap();
    assert_eq!(predator_prey_serialize(&restored).unwrap(), serialized);
}

#[test]
fn grid_press_cycle_exact_world_tile() {
    let mut c = ctx();
    let s = empty();
    let s = predator_prey_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.tiles[grid_index(2, 3)], GRASS);
    assert_eq!(s.trigger_types[grid_index(2, 3)], TileTriggerType::Stable);
    let s = predator_prey_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.tiles[grid_index(2, 3)], HERBIVORE);
    assert_eq!(s.trigger_types[grid_index(2, 3)], TileTriggerType::Activate);
    let s = predator_prey_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.tiles[grid_index(2, 3)], PREDATOR);
    assert_eq!(s.trigger_types[grid_index(2, 3)], TileTriggerType::Activate);
    let s = predator_prey_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.tiles[grid_index(2, 3)], EMPTY);
    assert_eq!(
        s.trigger_types[grid_index(2, 3)],
        TileTriggerType::Deactivate
    );
}

#[test]
fn trigger_precedence_preserves_deactivation_and_quiet_grass() {
    let index = grid_index(3, 3);
    let mut previous = vec![EMPTY; TILE_COUNT];
    let mut next = vec![EMPTY; TILE_COUNT];

    previous[index] = HERBIVORE;
    let deactivated = triggers(&previous, &next, &[index], &[index]);
    assert_eq!(deactivated[index], TileTriggerType::Deactivate);

    next[index] = GRASS;
    let animal_departure = triggers(&previous, &next, &[], &[]);
    assert_eq!(animal_departure[index], TileTriggerType::Deactivate);

    previous[index] = GRASS;
    let burst = triggers(&previous, &next, &[index], &[]);
    assert_eq!(burst[index], TileTriggerType::Activate);

    let movement = triggers(&previous, &next, &[], &[index]);
    assert_eq!(movement[index], TileTriggerType::Activate);

    let stable = triggers(&previous, &next, &[], &[]);
    assert_eq!(stable[index], TileTriggerType::Stable);
    assert_eq!(stable[grid_index(4, 3)], TileTriggerType::None);
}

#[test]
fn relief_reseed_uses_final_tile_before_assigning_trigger() {
    let mut c = ctx();
    let mut state = empty();
    state.tiles.fill(PREDATOR);
    state.energy.fill(state.starve_ticks);

    let next = predator_prey_on_tick(state, &mut c);

    assert_eq!(next.tiles[0], HERBIVORE);
    assert_eq!(next.trigger_types[0], TileTriggerType::Activate);
}

#[test]
fn nudge_and_reseed_do_not_leave_stale_deactivations() {
    let mut c = ctx();
    let state = empty();
    let next = predator_prey_on_tick(state, &mut c);

    assert!(next
        .trigger_types
        .iter()
        .all(|trigger| *trigger != TileTriggerType::Deactivate));
    assert_eq!(next.tiles[0], HERBIVORE);
    assert_eq!(next.tiles[1], PREDATOR);
    assert_eq!(next.trigger_types[0], TileTriggerType::Activate);
    assert_eq!(next.trigger_types[1], TileTriggerType::Activate);
}

#[test]
fn grass_regrowth_stable_and_herbivore_eats_without_burst() {
    let mut c = ctx();
    let mut s = empty();
    s.grass_grow_chance_pct = 100;
    let grown = predator_prey_on_tick(s, &mut c);
    assert!(grown
        .tiles
        .iter()
        .enumerate()
        .any(|(i, tile)| *tile == GRASS && grown.trigger_types[i] == TileTriggerType::Stable));
    let mut s = empty();
    s.tiles[grid_index(1, 1)] = HERBIVORE;
    s.energy[grid_index(1, 1)] = 3;
    s.tiles[grid_index(1, 2)] = GRASS;
    let n = predator_prey_on_tick(s, &mut c);
    assert_eq!(n.tiles[grid_index(1, 2)], HERBIVORE);
    assert_eq!(n.trigger_types[grid_index(1, 2)], TileTriggerType::Activate);
    assert_eq!(n.trigger_types[grid_index(2, 2)], TileTriggerType::None);
}

#[test]
fn predator_eat_burst_clips_and_does_not_mutate_or_repeat() {
    let mut c = ctx();
    let mut s = empty();
    s.tiles[grid_index(0, 1)] = PREDATOR;
    s.energy[grid_index(0, 1)] = 3;
    s.tiles[grid_index(0, 2)] = HERBIVORE;
    let n = predator_prey_on_tick(s, &mut c);
    assert_eq!(n.tiles[grid_index(1, 2)], EMPTY);
    assert_eq!(n.trigger_types[grid_index(0, 2)], TileTriggerType::Activate);
    assert_eq!(n.trigger_types[grid_index(1, 2)], TileTriggerType::Activate);
    let n2 = predator_prey_on_tick(n, &mut c);
    assert_ne!(
        n2.trigger_types[grid_index(1, 2)],
        TileTriggerType::Activate
    );
}

#[test]
fn births_and_reseeded_animals_force_activate() {
    let mut c = ctx();
    let mut s = empty();
    s.herbivore_reproduce_pct = 100;
    s.tiles[grid_index(1, 1)] = HERBIVORE;
    s.energy[grid_index(1, 1)] = 3;
    s.tiles[grid_index(1, 2)] = GRASS;
    let n = predator_prey_on_tick(s, &mut c);
    assert_eq!(n.tiles[grid_index(1, 1)], HERBIVORE);
    assert_eq!(n.trigger_types[grid_index(1, 1)], TileTriggerType::Activate);

    let mut s = empty();
    s.tiles[0] = HERBIVORE;
    s.energy[0] = 3;
    s.tiles[1] = PREDATOR;
    s.energy[1] = 3;
    let n = predator_prey_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "reseedEcosystem".into(),
        }),
        &mut c,
    );
    assert_eq!(n.trigger_types[0], TileTriggerType::Activate);
    assert_eq!(n.trigger_types[1], TileTriggerType::Activate);
}

#[test]
fn starvation_conflict_nonwrap_and_reseed() {
    let mut c = ctx();
    let mut s = empty();
    s.tiles[grid_index(0, 0)] = HERBIVORE;
    s.energy[grid_index(0, 0)] = 1;
    s.tiles[grid_index(7, 0)] = HERBIVORE;
    s.energy[grid_index(7, 0)] = 3;
    s.tiles[grid_index(7, 7)] = PREDATOR;
    s.energy[grid_index(7, 7)] = 3;
    let n = predator_prey_on_tick(s, &mut c);
    assert_eq!(
        n.trigger_types[grid_index(0, 0)],
        TileTriggerType::Deactivate
    );
    let mut s = empty();
    s.tiles[grid_index(1, 0)] = PREDATOR;
    s.energy[grid_index(1, 0)] = 3;
    s.tiles[grid_index(0, 1)] = PREDATOR;
    s.energy[grid_index(0, 1)] = 3;
    s.tiles[grid_index(0, 0)] = HERBIVORE;
    let n = predator_prey_on_tick(s, &mut c);
    assert!(n.tiles.iter().filter(|x| **x == PREDATOR).count() >= 1);
    let mut s = empty();
    s.tiles[0] = GRASS;
    let n = predator_prey_on_tick(s, &mut c);
    assert!(n.tiles.contains(&HERBIVORE));
    assert!(n.tiles.contains(&PREDATOR));
}

#[test]
fn full_grass_grid_reopens_without_firehose() {
    let mut c = ctx();
    let mut s = empty();
    s.tiles.fill(GRASS);
    s.grass_grow_chance_pct = 100;

    let n = predator_prey_on_tick(s, &mut c);

    assert!(n.tiles.contains(&EMPTY));
    assert!(
        n.trigger_types
            .iter()
            .filter(|trigger| **trigger == TileTriggerType::Deactivate)
            .count()
            <= 4
    );
}

#[test]
fn static_visibility_nudge_removal_deactivates_removed_tile() {
    let previous = vec![GRASS; TILE_COUNT];
    let mut next = previous.clone();
    let mut energy = vec![0; TILE_COUNT];

    let removed = nudge_static_visibility(&previous, &mut next, &mut energy);
    assert_eq!(removed, Some(0));
    let trigger_types = triggers_with_deactivations(&previous, &next, &[], &[], &[0]);
    assert_eq!(next[0], EMPTY);
    assert_eq!(trigger_types[0], TileTriggerType::Deactivate);
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
        empty_run = if model.tiles.iter().all(|tile| !*tile) {
            empty_run + 1
        } else {
            0
        };
        full_run = if model.tiles.iter().all(|tile| *tile) {
            full_run + 1
        } else {
            0
        };
        assert!(empty_run <= 1);
        assert!(full_run <= 1);
    }
}
