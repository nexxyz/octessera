use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn movement_uses_eighth_cell_units_and_clamps_walls() {
    let mut state = bubbles_init(
        serde_json::json!({ "spawnInterval": 0, "drift": 0, "current": 8, "buoyancy": 2 }),
    )
    .unwrap();
    state.bubbles.push(Bubble {
        x: 6 * SUBSTEPS,
        y: 0,
        radius: 1,
    });
    let ticked = bubbles_on_tick(state, &mut context());
    assert_eq!(ticked.bubbles[0].x, 7 * SUBSTEPS);
    assert_eq!(ticked.bubbles[0].y, 16);
}

#[test]
fn spawn_interval_count_and_cap_are_respected() {
    let mut state =
        bubbles_init(serde_json::json!({ "spawnInterval": 2, "spawnCount": 3, "maxBubbles": 5 }))
            .unwrap();
    state = bubbles_on_tick(state, &mut context());
    assert!(state.bubbles.len() <= 3 && !state.bubbles.is_empty());
    state = bubbles_on_tick(state, &mut context());
    let after_off_step = state.bubbles.len();
    state = bubbles_on_tick(state, &mut context());
    assert!(state.bubbles.len() <= after_off_step + 3);
    assert!(state.bubbles.len() <= 5);
}

#[test]
fn defaults_float_upward_and_spawn_visibly_within_cap() {
    let mut state = bubbles_init(serde_json::json!({})).unwrap();
    state.spawn_interval = 0;
    state.bubbles.push(Bubble {
        x: 4 * SUBSTEPS,
        y: 0,
        radius: 1,
    });
    let start = state.bubbles[0].clone();
    state = bubbles_on_tick(state, &mut context());
    let moved = state
        .bubbles
        .iter()
        .find(|bubble| bubble.x == start.x)
        .unwrap();
    assert!(moved.y - start.y > (moved.x - start.x).abs());

    state = bubbles_init(serde_json::json!({})).unwrap();
    let cap = state.max_bubbles;
    for _ in 0..12 {
        state = bubbles_on_tick(state, &mut context());
    }
    assert!(!state.bubbles.is_empty());
    assert!(state.bubbles.len() <= cap);
}

#[test]
fn render_shapes_match_size_levels() {
    assert_eq!(
        bubble_cells(&Bubble {
            x: SUBSTEPS,
            y: SUBSTEPS,
            radius: 1
        }),
        vec![(1, 1)]
    );
    let plus = bubble_cells(&Bubble {
        x: SUBSTEPS,
        y: SUBSTEPS,
        radius: 2,
    });
    assert!(plus.contains(&(1, 1)) && plus.contains(&(0, 1)) && plus.contains(&(1, 2)));
    let hollow = bubble_cells(&Bubble {
        x: SUBSTEPS,
        y: SUBSTEPS,
        radius: 3,
    });
    assert!(!hollow.contains(&(1, 1)) && hollow.contains(&(0, 1)) && hollow.contains(&(2, 1)));
    assert!(!hollow.contains(&(0, 0)) && !hollow.contains(&(2, 2)));
}

#[test]
fn input_spawns_render_immediately_and_deserialize_is_bounded() {
    let state = bubbles_init(serde_json::json!({ "spawnInterval": 0, "drift": 0 })).unwrap();
    let input = bubbles_on_input(state, DeviceInput::GridPress { x: 2, y: 0 }, &mut context());
    assert!(input.cells[grid_index(2, 0)]);
    assert_eq!(
        input.trigger_types[grid_index(2, 0)],
        CellTriggerType::Activate
    );

    let oversized = serde_json::json!({
        "bubbles": (0..80)
            .map(|index| serde_json::json!({ "x": index * SUBSTEPS, "y": 0, "radius": 9 }))
            .collect::<Vec<_>>(),
        "cells": [true],
        "triggerTypes": [],
        "minRadius": 1,
        "maxRadius": 4,
        "spawnInterval": 0,
        "spawnStep": 99,
        "spawnCount": 20,
        "drift": 99,
        "current": -99,
        "buoyancy": 0,
        "maxBubbles": 8,
        "tickCounter": 123
    });
    let restored = bubbles_deserialize(oversized).unwrap();
    assert_eq!(restored.bubbles.len(), 8);
    assert_eq!(restored.cells.len(), CELL_COUNT);
    assert_eq!(restored.trigger_types.len(), CELL_COUNT);
    assert!(restored.bubbles.iter().all(|bubble| bubble.radius <= 4));
}

#[test]
fn merge_repeats_in_stable_order_and_despawns_above_top() {
    let mut state = bubbles_init(
        serde_json::json!({ "spawnInterval": 0, "drift": 0, "buoyancy": 1, "maxRadius": 4 }),
    )
    .unwrap();
    state.bubbles = vec![
        Bubble {
            x: 0,
            y: 0,
            radius: 1,
        },
        Bubble {
            x: SUBSTEPS,
            y: 0,
            radius: 1,
        },
        Bubble {
            x: 2 * SUBSTEPS,
            y: 0,
            radius: 1,
        },
    ];
    let ticked = bubbles_on_tick(state, &mut context());
    assert_eq!(ticked.bubbles.len(), 1);
    assert_eq!(ticked.bubbles[0].radius, 3);

    let mut top =
        bubbles_init(serde_json::json!({ "spawnInterval": 0, "drift": 0, "buoyancy": 8 })).unwrap();
    top.bubbles.push(Bubble {
        x: 0,
        y: (GRID_HEIGHT as i32 + 2) * SUBSTEPS,
        radius: 1,
    });
    assert!(bubbles_on_tick(top, &mut context()).bubbles.is_empty());
}

#[test]
fn config_menu_normalization_serialization_and_render_contract() {
    let state = bubbles_init(
        serde_json::json!({ "minRadius": 9, "maxRadius": 1, "current": 99, "buoyancy": 0 }),
    )
    .unwrap();
    assert_eq!(
        (
            state.min_radius,
            state.max_radius,
            state.current,
            state.buoyancy
        ),
        (4, 4, 8, 1)
    );
    assert_eq!(bubbles_render_model(&state).name, "bubbles");
    assert_eq!(bubbles_config_menu().last().unwrap().key, "addBubble");
    let serialized = bubbles_serialize(&state).unwrap();
    assert!(serialized.get("tickCounter").is_none());
}
