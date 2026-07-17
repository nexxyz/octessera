use super::*;
use crate::behavior::{BehaviorActionInput, BehaviorContext, CellTriggerType, DeviceInput};
use serde_json::json;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_normalize_serialize_restore_quiet() {
    let menu = maze_growth_config_menu();
    assert_eq!(menu[0].key, "carvePct");
    assert_eq!(menu[1].key, "collapseAge");
    assert_eq!(menu[2].key, "walkerCount");
    assert_eq!(menu[3].key, "restartMaze");
    assert_eq!(menu[4].key, "collapseMaze");
    let state = maze_growth_init(json!({"cells":[9,2],"visited":[2],"ages":[300],"walkers":[1,1,99],"walkerCount":2,"carvePct":200,"collapseAge":0})).unwrap();
    assert_eq!(state.cells[0], 3);
    assert_eq!(state.cells[1], WALKER);
    assert_eq!(state.visited[0], 1);
    assert_eq!(state.ages[0], 255);
    assert_eq!(state.carve_pct, 100);
    assert_eq!(state.collapse_age, 1);
    let many = maze_growth_init(
        json!({"cells":[1,1,1,1,1,1,1,1],"walkers":[0,1,2,3,4,5,6,7],"walkerCount":8}),
    )
    .unwrap();
    assert_eq!(many.walker_count, 8);
    assert_eq!(many.walkers.len(), 8);
    let many_value = maze_growth_serialize(&many).unwrap();
    let many_restored = maze_growth_deserialize(many_value.clone()).unwrap();
    assert_eq!(maze_growth_serialize(&many_restored).unwrap(), many_value);
    let orphaned =
        maze_growth_init(json!({"cells":[3,3,3],"walkers":[0],"walkerCount":2})).unwrap();
    assert_eq!(orphaned.walkers, vec![0, 1]);
    assert_eq!(orphaned.cells[2], PATH);
    assert!(state
        .trigger_types
        .iter()
        .all(|t| *t != CellTriggerType::Activate));
    let value = maze_growth_serialize(&state).unwrap();
    assert!(value.get("triggerTypes").is_none());
    assert!(value.get("tickCounter").is_none());
    let restored = maze_growth_deserialize(value.clone()).unwrap();
    assert_eq!(maze_growth_serialize(&restored).unwrap(), value);
}

#[test]
fn grid_restart_collapse_and_walker_entry() {
    let mut ctx = context();
    let state = maze_growth_init(json!({})).unwrap();
    let path = maze_growth_on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(path.cells[grid_index(0, 0)], PATH);
    assert_eq!(
        path.trigger_types[grid_index(0, 0)],
        CellTriggerType::Activate
    );
    let wall = maze_growth_on_input(path, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(
        wall.trigger_types[grid_index(0, 0)],
        CellTriggerType::Deactivate
    );
    let restarted = maze_growth_on_input(
        wall,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "restartMaze".into(),
        }),
        &mut ctx,
    );
    assert_eq!(restarted.cells[grid_index(3, 3)], WALKER);
    assert_eq!(
        restarted.trigger_types[grid_index(3, 3)],
        CellTriggerType::Activate
    );
    let ticked = maze_growth_on_tick(restarted, &mut ctx);
    assert!(ticked.trigger_types.contains(&CellTriggerType::Activate));
    let collapsed = maze_growth_on_input(
        ticked,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "collapseMaze".into(),
        }),
        &mut ctx,
    );
    for walker in &collapsed.walkers {
        assert_eq!(collapsed.cells[*walker], WALKER);
    }
}

#[test]
fn default_init_seeds_visible_maze_quietly_and_grows() {
    let mut ctx = context();
    let state = maze_growth_init(json!({})).unwrap();
    assert!(state.cells.iter().any(|cell| *cell == WALKER));
    assert!(state.cells.iter().any(|cell| *cell == FRONTIER));
    assert!(state.cells.iter().any(|cell| *cell == PATH));
    assert!(state
        .trigger_types
        .iter()
        .all(|trigger| *trigger != CellTriggerType::Activate));
    let visible = state.cells.iter().filter(|cell| **cell != WALL).count();
    let ticked = maze_growth_on_tick(state, &mut ctx);
    assert!(ticked.cells.iter().filter(|cell| **cell != WALL).count() >= visible);
}

#[test]
fn deserialize_all_wall_saved_state_stays_empty() {
    let state = maze_growth_deserialize(json!({
        "cells": vec![WALL; CELL_COUNT],
        "visited": vec![0; CELL_COUNT],
        "ages": vec![0; CELL_COUNT],
        "walkers": [],
        "walkerCount": 2
    }))
    .unwrap();
    assert!(state.cells.iter().all(|cell| *cell == WALL));
    assert!(state.walkers.is_empty());
}

#[test]
fn default_init_resets_seeded_cell_ages() {
    let state = maze_growth_init(json!({
        "cells": vec![WALL; CELL_COUNT],
        "visited": vec![0; CELL_COUNT],
        "ages": vec![64; CELL_COUNT],
        "walkers": [],
        "collapseAge": 1
    }))
    .unwrap();
    for (index, cell) in state.cells.iter().enumerate() {
        if *cell != WALL {
            assert_eq!(state.ages[index], 0);
        }
    }
}

#[test]
fn carving_no_wrap_reservation_and_collapse() {
    let mut ctx = context();
    let mut cells = vec![WALL; CELL_COUNT];
    let mut visited = vec![0; CELL_COUNT];
    cells[grid_index(0, 0)] = FRONTIER;
    visited[grid_index(0, 0)] = 1;
    let state = maze_growth_init(
        json!({"cells":cells,"visited":visited,"carvePct":100,"walkerCount":1,"walkers":[]}),
    )
    .unwrap();
    let ticked = maze_growth_on_tick(state, &mut ctx);
    assert_eq!(ticked.cells[grid_index(7, 0)], WALL);
    assert_eq!(ticked.cells.iter().filter(|c| **c == FRONTIER).count(), 1);
    let mut cells = vec![WALL; CELL_COUNT];
    let mut visited = vec![0; CELL_COUNT];
    cells[grid_index(1, 1)] = FRONTIER;
    visited[grid_index(1, 1)] = 1;
    visited[grid_index(1, 2)] = 1;
    cells[grid_index(2, 1)] = PATH;
    let state = maze_growth_init(
        json!({"cells":cells,"visited":visited,"carvePct":100,"walkerCount":1,"walkers":[]}),
    )
    .unwrap();
    let ticked = maze_growth_on_tick(state, &mut ctx);
    assert_ne!(ticked.cells[grid_index(1, 2)], FRONTIER);
    assert_ne!(ticked.cells[grid_index(2, 1)], FRONTIER);
    assert_eq!(ticked.cells[grid_index(1, 0)], FRONTIER);
    let mut cells = vec![WALL; CELL_COUNT];
    cells[grid_index(1, 1)] = WALKER;
    cells[grid_index(1, 2)] = PATH;
    cells[grid_index(2, 1)] = WALKER;
    cells[grid_index(2, 2)] = PATH;
    let state =
        maze_growth_init(json!({"cells":cells,"walkers":[9,10],"walkerCount":2,"carvePct":0}))
            .unwrap();
    let ticked = maze_growth_on_tick(state, &mut ctx);
    assert_eq!(ticked.walkers.len(), 2);
    assert_ne!(ticked.walkers[0], ticked.walkers[1]);
    let mut cells = vec![WALL; CELL_COUNT];
    let mut ages = vec![0; CELL_COUNT];
    cells[grid_index(1, 1)] = PATH;
    ages[grid_index(1, 1)] = 64;
    let state = maze_growth_init(json!({"cells":cells,"ages":ages,"collapseAge":1,"carvePct":100,"walkerCount":1,"walkers":[]})).unwrap();
    let ticked = maze_growth_on_tick(state, &mut ctx);
    assert_eq!(ticked.cells[grid_index(1, 1)], WALL);
    assert_eq!(
        ticked.trigger_types[grid_index(1, 1)],
        CellTriggerType::Deactivate
    );
}
