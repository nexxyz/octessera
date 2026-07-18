use super::*;

#[test]
fn empty_config_seeds_quiet_default_oscillator_but_deserialized_empty_stays_empty() {
    let state = init(serde_json::json!({})).unwrap();
    assert_eq!(state.cells.iter().filter(|cell| **cell).count(), 3);
    assert!(!state.cells[grid_index(2, 3)]);
    assert!(!state.trigger_types.contains(&CellTriggerType::Activate));
    let restored = deserialize(serde_json::json!({
        "width": GRID_WIDTH,
        "height": GRID_HEIGHT,
        "cells": [],
        "randomCellsPerTick": 0,
        "randomTickInterval": 1,
        "gliderSpawnInterval": 0,
        "spawnStep": 0,
        "triggerTypes": []
    }))
    .unwrap();
    assert!(restored.cells.iter().all(|cell| !cell));
}

#[test]
fn default_seed_does_not_become_terminal_static_over_detector_window() {
    let mut context = BehaviorContext::new(120.0);
    let mut state = init(serde_json::json!({})).unwrap();
    let mut static_frames = 0;
    for _ in 0..300 {
        let previous = state.cells.clone();
        state = on_tick(state, &mut context);
        if state.cells == previous {
            static_frames += 1;
        } else {
            static_frames = 0;
        }
        assert!(static_frames <= 2);
    }
}

#[test]
fn blinker_oscillates() {
    let mut state = init(serde_json::json!({ "cells": [] })).unwrap();
    state.cells[grid_index(2, 3)] = true;
    state.cells[grid_index(3, 3)] = true;
    state.cells[grid_index(4, 3)] = true;
    let mut context = BehaviorContext::new(120.0);
    let next = on_tick(state, &mut context);
    assert!(next.cells[grid_index(3, 2)]);
    assert!(next.cells[grid_index(3, 3)]);
    assert!(next.cells[grid_index(3, 4)]);
    assert!(!next.cells[grid_index(2, 3)]);
    assert!(context.emitted_events.is_empty());
}

#[test]
fn twelve_live_cells_do_not_emit_direct_notes() {
    let mut state = init(serde_json::json!({ "cells": [] })).unwrap();
    for (x, y) in [
        (1, 1),
        (2, 1),
        (1, 2),
        (2, 2),
        (5, 1),
        (6, 1),
        (5, 2),
        (6, 2),
        (1, 5),
        (2, 5),
        (1, 6),
        (2, 6),
    ] {
        state.cells[grid_index(x, y)] = true;
    }
    let mut context = BehaviorContext::new(120.0);

    let next = on_tick(state, &mut context);

    assert_eq!(next.cells.iter().filter(|cell| **cell).count(), 12);
    assert!(context.emitted_events.is_empty());
}

#[test]
fn block_is_stable_and_grid_press_toggles_cell() {
    let mut state = init(serde_json::json!({ "cells": [] })).unwrap();
    for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
        state.cells[grid_index(x, y)] = true;
    }
    let next = on_tick(state.clone(), &mut BehaviorContext::new(120.0));
    assert_eq!(next.cells, state.cells);
    assert_eq!(
        next.trigger_types[grid_index(2, 2)],
        CellTriggerType::Stable
    );

    let mut context = BehaviorContext::new(120.0);
    let toggled = on_input(
        init(serde_json::json!({ "cells": [] })).unwrap(),
        DeviceInput::GridPress { x: 2, y: 3 },
        &mut context,
    );
    assert!(toggled.cells[grid_index(2, 3)]);
    let toggled = on_input(toggled, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert!(!toggled.cells[grid_index(2, 3)]);
}

#[test]
fn render_config_and_serialization_match_contract() {
    let mut state = init(serde_json::json!({ "cells": [] })).unwrap();
    state.cells[grid_index(1, 1)] = true;
    let model = render_model(&state);
    assert_eq!(model.name, "game of life");
    assert_eq!(model.status_line, "Gen 0");
    assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);

    let menu = config_menu(&state);
    assert_eq!(
        menu.iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec![
            "randomCellsPerTick",
            "randomTickInterval",
            "gliderSpawnInterval",
            "spawnStep",
            "spawnRandom",
            "spawnGlider"
        ]
    );

    let raw = serialize(&state).unwrap();
    assert_eq!(deserialize(raw).unwrap(), state);
}

#[test]
fn spawn_glider_action_adds_glider_pattern() {
    let mut context = BehaviorContext::new(120.0);
    let state = on_input(
        init(serde_json::json!({ "cells": [] })).unwrap(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "spawnGlider".into(),
        }),
        &mut context,
    );
    for (x, y) in [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
        assert!(state.cells[grid_index(x, y)]);
        assert_eq!(
            state.trigger_types[grid_index(x, y)],
            CellTriggerType::Activate
        );
    }
}

#[test]
fn spawn_random_action_and_random_tick_spawn_activate_cells() {
    let mut context = BehaviorContext::new(120.0);
    let empty = init(serde_json::json!({ "cells": [] })).unwrap();
    let spawned = on_input(
        empty.clone(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "spawnRandom".into(),
        }),
        &mut context,
    );
    assert!(spawned.cells.iter().filter(|cell| **cell).count() > 0);
    assert!(spawned
        .trigger_types
        .iter()
        .any(|trigger| *trigger == CellTriggerType::Activate));

    let random_tick = init(serde_json::json!({
        "cells": [],
        "randomCellsPerTick": 4,
        "randomTickInterval": 1,
        "spawnStep": 0
    }))
    .unwrap();
    let ticked = on_tick(random_tick, &mut context);
    assert!(ticked.cells.iter().filter(|cell| **cell).count() > 0);
    assert!(ticked
        .trigger_types
        .iter()
        .any(|trigger| *trigger == CellTriggerType::Activate));

    let ignored = on_input(
        spawned.clone(),
        DeviceInput::GridPress { x: 99, y: 0 },
        &mut context,
    );
    assert_eq!(ignored, spawned);
}

#[test]
fn glider_interval_is_disabled_by_default_and_spawn_step_delays_it() {
    let mut context = BehaviorContext::new(120.0);
    let default_state = init(serde_json::json!({ "cells": [] })).unwrap();
    let default_next = on_tick(default_state, &mut context);
    assert!(default_next.cells.iter().all(|cell| !cell));

    let delayed = init(serde_json::json!({
        "cells": [],
        "gliderSpawnInterval": 2,
        "spawnStep": 1
    }))
    .unwrap();
    let first = on_tick(delayed, &mut context);
    assert!(first.cells.iter().all(|cell| !cell));

    let second = on_tick(first, &mut context);
    for (x, y) in [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
        assert!(second.cells[grid_index(x, y)]);
        assert_eq!(
            second.trigger_types[grid_index(x, y)],
            CellTriggerType::Activate
        );
    }
}

#[test]
fn glider_moves_diagonally_after_four_generations() {
    let mut state = init(serde_json::json!({ "cells": [] })).unwrap();
    for (x, y) in [(2, 1), (3, 2), (1, 3), (2, 3), (3, 3)] {
        state.cells[grid_index(x, y)] = true;
    }
    let mut context = BehaviorContext::new(120.0);
    for _ in 0..4 {
        state = on_tick(state, &mut context);
    }
    for (x, y) in [(3, 2), (4, 3), (2, 4), (3, 4), (4, 4)] {
        assert!(state.cells[grid_index(x, y)]);
    }
}
