use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

fn active_count(state: &TwinkleState) -> usize {
    state.tiles.iter().filter(|tile| **tile).count()
}

fn state_with_tiles(tiles: &[usize], config: Value) -> TwinkleState {
    let mut object = config.as_object().cloned().unwrap_or_default();
    object.insert(
        "tiles".into(),
        Value::Array(
            (0..TILE_COUNT)
                .map(|index| Value::from(tiles.contains(&index)))
                .collect(),
        ),
    );
    twinkle_deserialize(Value::Object(object)).unwrap()
}

#[test]
fn defaults_ranges_menu_and_palette_match_frozen_contract() {
    let state = twinkle_init(Value::Null).unwrap();
    assert_eq!(active_count(&state), 3);
    assert_eq!(state.density, 3);
    assert_eq!(state.birth_chance_pct, 70);
    assert_eq!(state.fade_chance_pct, 35);
    assert_eq!(state.star_life, 8);
    assert_eq!(state.cluster_bias_pct, 40);
    assert_eq!(state.seed, 1);
    assert_eq!(state.rng_counter, 0);

    let menu = twinkle_config_menu();
    assert_eq!(
        menu.iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        [
            "density",
            "birthChancePct",
            "fadeChancePct",
            "starLife",
            "clusterBiasPct",
            "seed",
            "reseedStars",
            "clearStars"
        ]
    );
    assert_eq!((menu[0].min, menu[0].max), (Some(1), Some(5)));
    assert_eq!((menu[3].min, menu[3].max), (Some(1), Some(32)));
    assert_eq!((menu[5].min, menu[5].max), (Some(0), Some(65535)));

    let model = twinkle_render_model(&state);
    assert_eq!(model.name, "twinkle");
    assert_eq!(model.palette.active, [80, 170, 255]);
    assert_eq!(model.palette.stable, [18, 48, 82]);
    assert_eq!(model.palette.inactive, [0, 0, 0]);
    assert_eq!(model.tiles.len(), TILE_COUNT);
}

#[test]
fn missing_tiles_seed_but_explicit_empty_stays_empty() {
    let seeded = twinkle_deserialize(serde_json::json!({"seed": 22})).unwrap();
    let empty = twinkle_deserialize(serde_json::json!({"tiles": [], "seed": 22})).unwrap();
    assert_eq!(active_count(&seeded), 3);
    assert_eq!(active_count(&empty), 0);
    assert!(empty.ages.iter().all(|age| *age == 0));
    assert!(empty
        .trigger_types
        .iter()
        .all(|trigger| *trigger == TileTriggerType::None));
}

#[test]
fn malformed_state_is_safe_and_normalized_deterministically() {
    let state = twinkle_deserialize(serde_json::json!({
        "tiles": [true, "bad", true, true, true, true, true, false],
        "ages": [255, -1, 99, "bad"],
        "density": -1,
        "birthChancePct": 999,
        "fadeChancePct": -1,
        "starLife": 999,
        "clusterBiasPct": -1,
        "seed": 999999,
        "rngCounter": -1,
        "triggerTypes": ["activate", "bad"]
    }))
    .unwrap();
    assert_eq!(state.tiles.len(), TILE_COUNT);
    assert_eq!(state.ages.len(), TILE_COUNT);
    assert_eq!(state.trigger_types.len(), TILE_COUNT);
    assert_eq!(state.tiles[..5], [true, false, false, false, false]);
    assert_eq!(state.ages[0], 255);
    assert_eq!(state.ages[1], 0);
    assert_eq!(state.ages[2], 0);
    assert_eq!(state.density, 1);
    assert_eq!(state.birth_chance_pct, 100);
    assert_eq!(state.fade_chance_pct, 0);
    assert_eq!(state.star_life, 32);
    assert_eq!(state.cluster_bias_pct, 0);
    assert_eq!(state.seed, u16::MAX);
    assert_eq!(state.rng_counter, 0);
    assert_eq!(state.trigger_types[0], TileTriggerType::Stable);
    assert!(state.trigger_types[1..2]
        .iter()
        .all(|trigger| *trigger == TileTriggerType::None));
    assert!(state.trigger_types[2..7]
        .iter()
        .all(|trigger| *trigger == TileTriggerType::Deactivate));
    assert!(state.trigger_types[7..]
        .iter()
        .all(|trigger| *trigger == TileTriggerType::None));
}

#[test]
fn cap_reduction_and_manual_grid_semantics_are_deterministic() {
    let capped = state_with_tiles(
        &[1, 3, 5],
        serde_json::json!({"density": 2, "birthChancePct": 0}),
    );
    assert_eq!(
        capped
            .tiles
            .iter()
            .enumerate()
            .filter_map(|(index, active)| (*active).then_some(index))
            .collect::<Vec<_>>(),
        [1, 3]
    );

    let mut state = state_with_tiles(&[], serde_json::json!({"density": 1}));
    state = twinkle_on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut context());
    assert!(state.tiles[grid_index(0, 0)]);
    assert_eq!(state.ages[0], 0);
    assert_eq!(state.trigger_types[0], TileTriggerType::Activate);
    let unchanged = twinkle_on_input(
        state.clone(),
        DeviceInput::GridPress { x: 7, y: 7 },
        &mut context(),
    );
    assert!(!unchanged.tiles[0]);
    assert!(unchanged.tiles[63]);
    assert_eq!(unchanged.trigger_types[0], TileTriggerType::Deactivate);
    assert_eq!(unchanged.trigger_types[63], TileTriggerType::Activate);
    let out_of_range = twinkle_on_input(
        unchanged.clone(),
        DeviceInput::GridPress { x: 8, y: 0 },
        &mut context(),
    );
    assert_eq!(out_of_range, unchanged);

    let state = twinkle_on_input(
        unchanged,
        DeviceInput::GridPress { x: 7, y: 7 },
        &mut context(),
    );
    assert_eq!(state.trigger_types[63], TileTriggerType::Deactivate);
    assert!(!state.tiles[63]);

    let state = state_with_tiles(&[0], serde_json::json!({"density": 1}));
    let state = twinkle_on_input(state, DeviceInput::GridPress { x: 7, y: 7 }, &mut context());
    assert!(!state.tiles[0]);
    assert!(state.tiles[63]);
    assert_eq!(state.trigger_types[0], TileTriggerType::Deactivate);
    assert_eq!(state.trigger_types[63], TileTriggerType::Activate);
}

#[test]
fn density_reduction_marks_removed_stars_as_deactivated() {
    let mut tiles = vec![false; TILE_COUNT];
    tiles[1] = true;
    tiles[3] = true;
    tiles[5] = true;
    let state = twinkle_init(serde_json::json!({
        "tiles": tiles.clone(),
        "density": 2
    }))
    .unwrap();
    assert_eq!(state.tiles.iter().filter(|tile| **tile).count(), 2);
    assert_eq!(state.trigger_types[1], TileTriggerType::Stable);
    assert_eq!(state.trigger_types[3], TileTriggerType::Stable);
    assert_eq!(state.trigger_types[5], TileTriggerType::Deactivate);

    let restored = twinkle_deserialize(serde_json::json!({
        "tiles": tiles,
        "density": 2
    }))
    .unwrap();
    assert_eq!(restored.trigger_types[5], TileTriggerType::Deactivate);
}

#[test]
fn valid_manual_press_recomputes_and_clears_stale_markers() {
    let mut state = state_with_tiles(&[0, 1], serde_json::json!({"density": 2}));
    state.trigger_types[7] = TileTriggerType::Deactivate;
    let state = twinkle_on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut context());
    assert_eq!(state.trigger_types[0], TileTriggerType::Deactivate);
    assert_eq!(state.trigger_types[1], TileTriggerType::Stable);
    assert_eq!(state.trigger_types[7], TileTriggerType::None);
}

#[test]
fn tick_honors_chances_life_cap_and_single_birth_death() {
    let state = state_with_tiles(
        &[0],
        serde_json::json!({
            "density": 1,
            "birthChancePct": 100,
            "fadeChancePct": 100,
            "starLife": 1,
            "clusterBiasPct": 0,
            "ages": [0]
        }),
    );
    let next = twinkle_on_tick(state, &mut context());
    assert_eq!(active_count(&next), 1);
    assert_eq!(
        next.trigger_types
            .iter()
            .filter(|t| **t == TileTriggerType::Deactivate)
            .count(),
        1
    );
    assert_eq!(
        next.trigger_types
            .iter()
            .filter(|t| **t == TileTriggerType::Activate)
            .count(),
        1
    );
    assert!(!next.tiles[0]);

    let state = state_with_tiles(
        &[0],
        serde_json::json!({
            "density": 2,
            "birthChancePct": 0,
            "fadeChancePct": 100,
            "starLife": 32
        }),
    );
    let next = twinkle_on_tick(state, &mut context());
    assert!(next.tiles[0]);
    assert_eq!(next.ages[0], 1);
    assert_eq!(next.rng_counter, 1);

    let state = state_with_tiles(
        &[0],
        serde_json::json!({
            "density": 1,
            "birthChancePct": 0,
            "fadeChancePct": 100,
            "starLife": 8,
            "ages": [7]
        }),
    );
    let next = twinkle_on_tick(state, &mut context());
    assert!(!next.tiles[0]);
    assert_eq!(next.ages[0], 0);

    let state = state_with_tiles(
        &[],
        serde_json::json!({"density": 5, "birthChancePct": 100, "fadeChancePct": 0}),
    );
    let next = twinkle_on_tick(state, &mut context());
    assert_eq!(active_count(&next), 1);

    let state = state_with_tiles(
        &[],
        serde_json::json!({
            "density": 2,
            "birthChancePct": 100,
            "fadeChancePct": 0,
            "clusterBiasPct": 100
        }),
    );
    assert_eq!(active_count(&twinkle_on_tick(state, &mut context())), 1);
}

#[test]
fn cluster_bias_uses_clipped_non_wrapping_neighborhoods() {
    for &(x, y) in &[(0, 0), (7, 7)] {
        let index = grid_index(x, y);
        let state = state_with_tiles(
            &[index],
            serde_json::json!({
                "density": 2,
                "birthChancePct": 100,
                "fadeChancePct": 0,
                "clusterBiasPct": 100
            }),
        );
        let next = twinkle_on_tick(state, &mut context());
        let born = next
            .tiles
            .iter()
            .enumerate()
            .find_map(|(index, active)| (*active && index != grid_index(x, y)).then_some(index))
            .unwrap();
        let bx = born % GRID_WIDTH;
        let by = born / GRID_WIDTH;
        assert!(bx.abs_diff(x) <= 1 && by.abs_diff(y) <= 1);
    }
}

#[test]
fn actions_and_transitions_are_replayable() {
    let mut state = state_with_tiles(
        &[],
        serde_json::json!({"density": 4, "seed": 99, "birthChancePct": 0}),
    );
    state = twinkle_on_input(
        state,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "reseedStars".into(),
        }),
        &mut context(),
    );
    let replay = twinkle_on_input(
        state_with_tiles(&[], serde_json::json!({"density": 4, "seed": 99})),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "reseedStars".into(),
        }),
        &mut context(),
    );
    assert_eq!(state.tiles, replay.tiles);
    assert_eq!(state.rng_counter, 0);
    assert_eq!(active_count(&state), 4);

    let cleared = twinkle_on_input(
        state,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "clearStars".into(),
        }),
        &mut context(),
    );
    assert_eq!(active_count(&cleared), 0);
    assert!(cleared.ages.iter().all(|age| *age == 0));
    assert_eq!(
        cleared
            .trigger_types
            .iter()
            .filter(|trigger| **trigger == TileTriggerType::Deactivate)
            .count(),
        4
    );
}

#[test]
fn serialization_round_trip_preserves_replay_and_only_serializes_frozen_state() {
    let mut state = state_with_tiles(
        &[2, 9],
        serde_json::json!({
            "density": 3,
            "birthChancePct": 100,
            "fadeChancePct": 0,
            "starLife": 8,
            "clusterBiasPct": 0,
            "seed": 123,
            "rngCounter": 17,
            "ages": [0,0,4,0,0,0,0,0,0,9]
        }),
    );
    state = twinkle_on_tick(state, &mut context());
    let serialized = twinkle_serialize(&state).unwrap();
    assert!(serialized.get("triggerTypes").is_none());
    assert!(serialized.get("tickCounter").is_none());
    assert_eq!(serialized["rngCounter"], state.rng_counter);
    let restored = twinkle_deserialize(serialized.clone()).unwrap();
    assert_eq!(twinkle_serialize(&restored).unwrap(), serialized);
    assert!(restored
        .trigger_types
        .iter()
        .zip(restored.tiles.iter())
        .all(|(trigger, tile)| {
            *trigger
                == if *tile {
                    TileTriggerType::Stable
                } else {
                    TileTriggerType::None
                }
        }));

    let mut left = restored.clone();
    let mut right = twinkle_deserialize(serialized).unwrap();
    for _ in 0..12 {
        left = twinkle_on_tick(left, &mut context());
        right = twinkle_on_tick(right, &mut context());
        assert_eq!(
            twinkle_serialize(&left).unwrap(),
            twinkle_serialize(&right).unwrap()
        );
    }
}
