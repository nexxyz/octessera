use super::*;
use crate::behavior::{BehaviorActionInput, BehaviorContext, DeviceInput, TileTriggerType};
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
    let state = maze_growth_init(json!({"tiles":[9,2],"visited":[2],"ages":[300],"walkers":[1,1,99],"walkerCount":2,"carvePct":200,"collapseAge":0})).unwrap();
    assert_eq!(state.tiles[0], 3);
    assert_eq!(state.tiles[1], WALKER);
    assert_eq!(state.visited[0], 1);
    assert_eq!(state.ages[0], 255);
    assert_eq!(state.carve_pct, 100);
    assert_eq!(state.collapse_age, 1);
    let many = maze_growth_init(
        json!({"tiles":[1,1,1,1,1,1,1,1],"walkers":[0,1,2,3,4,5,6,7],"walkerCount":8}),
    )
    .unwrap();
    assert_eq!(many.walker_count, 8);
    assert_eq!(many.walkers.len(), 8);
    let many_value = maze_growth_serialize(&many).unwrap();
    let many_restored = maze_growth_deserialize(many_value.clone()).unwrap();
    assert_eq!(maze_growth_serialize(&many_restored).unwrap(), many_value);
    let orphaned =
        maze_growth_init(json!({"tiles":[3,3,3],"walkers":[0],"walkerCount":2})).unwrap();
    assert_eq!(orphaned.walkers, vec![0, 1]);
    assert_eq!(orphaned.tiles[2], PATH);
    assert!(state
        .trigger_types
        .iter()
        .all(|t| *t != TileTriggerType::Activate));
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
    assert_eq!(path.tiles[grid_index(0, 0)], PATH);
    assert_eq!(
        path.trigger_types[grid_index(0, 0)],
        TileTriggerType::Activate
    );
    let wall = maze_growth_on_input(path, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(
        wall.trigger_types[grid_index(0, 0)],
        TileTriggerType::Deactivate
    );
    let restarted = maze_growth_on_input(
        wall,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "restartMaze".into(),
        }),
        &mut ctx,
    );
    assert_eq!(restarted.tiles[grid_index(3, 3)], WALKER);
    assert_eq!(
        restarted.trigger_types[grid_index(3, 3)],
        TileTriggerType::Activate
    );
    let ticked = maze_growth_on_tick(restarted, &mut ctx);
    assert!(ticked.trigger_types.contains(&TileTriggerType::Activate));
    let collapsed = maze_growth_on_input(
        ticked,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "collapseMaze".into(),
        }),
        &mut ctx,
    );
    for walker in &collapsed.walkers {
        assert_eq!(collapsed.tiles[*walker], WALKER);
    }
}

#[test]
fn default_init_seeds_visible_maze_quietly_and_grows() {
    let mut ctx = context();
    let state = maze_growth_init(json!({})).unwrap();
    assert!(state.tiles.contains(&WALKER));
    assert!(state.tiles.contains(&FRONTIER));
    assert!(state.tiles.contains(&PATH));
    assert!(state
        .trigger_types
        .iter()
        .all(|trigger| *trigger != TileTriggerType::Activate));
    let visible = state.tiles.iter().filter(|tile| **tile != WALL).count();
    let ticked = maze_growth_on_tick(state, &mut ctx);
    assert!(ticked.tiles.iter().filter(|tile| **tile != WALL).count() >= visible);
}

#[test]
fn deserialize_all_wall_saved_state_stays_empty() {
    let state = maze_growth_deserialize(json!({
        "tiles": vec![WALL; TILE_COUNT],
        "visited": vec![0; TILE_COUNT],
        "ages": vec![0; TILE_COUNT],
        "walkers": [],
        "walkerCount": 2
    }))
    .unwrap();
    assert!(state.tiles.iter().all(|tile| *tile == WALL));
    assert!(state.walkers.is_empty());
}

#[test]
fn default_init_resets_seeded_tile_ages() {
    let state = maze_growth_init(json!({
        "tiles": vec![WALL; TILE_COUNT],
        "visited": vec![0; TILE_COUNT],
        "ages": vec![64; TILE_COUNT],
        "walkers": [],
        "collapseAge": 1
    }))
    .unwrap();
    for (index, tile) in state.tiles.iter().enumerate() {
        if *tile != WALL {
            assert_eq!(state.ages[index], 0);
        }
    }
}

#[test]
fn carving_no_wrap_reservation_and_collapse() {
    let mut ctx = context();
    let mut tiles = vec![WALL; TILE_COUNT];
    let mut visited = vec![0; TILE_COUNT];
    tiles[grid_index(0, 0)] = FRONTIER;
    visited[grid_index(0, 0)] = 1;
    let state = maze_growth_init(
        json!({"tiles":tiles,"visited":visited,"carvePct":100,"walkerCount":1,"walkers":[]}),
    )
    .unwrap();
    let ticked = maze_growth_on_tick(state, &mut ctx);
    assert_eq!(ticked.tiles[grid_index(7, 0)], WALL);
    assert_eq!(ticked.tiles.iter().filter(|c| **c == FRONTIER).count(), 1);
    let mut tiles = vec![WALL; TILE_COUNT];
    let mut visited = vec![0; TILE_COUNT];
    tiles[grid_index(1, 1)] = FRONTIER;
    visited[grid_index(1, 1)] = 1;
    visited[grid_index(1, 2)] = 1;
    tiles[grid_index(2, 1)] = PATH;
    let state = maze_growth_init(
        json!({"tiles":tiles,"visited":visited,"carvePct":100,"walkerCount":1,"walkers":[]}),
    )
    .unwrap();
    let ticked = maze_growth_on_tick(state, &mut ctx);
    assert_ne!(ticked.tiles[grid_index(1, 2)], FRONTIER);
    assert_ne!(ticked.tiles[grid_index(2, 1)], FRONTIER);
    assert_eq!(ticked.tiles[grid_index(1, 0)], FRONTIER);
    let mut tiles = vec![WALL; TILE_COUNT];
    tiles[grid_index(1, 1)] = WALKER;
    tiles[grid_index(1, 2)] = PATH;
    tiles[grid_index(2, 1)] = WALKER;
    tiles[grid_index(2, 2)] = PATH;
    let state =
        maze_growth_init(json!({"tiles":tiles,"walkers":[9,10],"walkerCount":2,"carvePct":0}))
            .unwrap();
    let ticked = maze_growth_on_tick(state, &mut ctx);
    assert_eq!(ticked.walkers.len(), 2);
    assert_ne!(ticked.walkers[0], ticked.walkers[1]);
    let mut tiles = vec![WALL; TILE_COUNT];
    let mut ages = vec![0; TILE_COUNT];
    tiles[grid_index(1, 1)] = PATH;
    ages[grid_index(1, 1)] = 64;
    let state = maze_growth_init(json!({"tiles":tiles,"ages":ages,"collapseAge":1,"carvePct":100,"walkerCount":1,"walkers":[]})).unwrap();
    let ticked = maze_growth_on_tick(state, &mut ctx);
    assert_eq!(ticked.tiles[grid_index(1, 1)], WALL);
    assert_eq!(
        ticked.trigger_types[grid_index(1, 1)],
        TileTriggerType::Deactivate
    );
}

#[test]
fn default_self_sustains_with_bounded_renewal() {
    let mut ctx = context();
    let mut state = maze_growth_init(json!({})).unwrap();
    let mut previous = maze_growth_render_model(&state).tiles;
    let mut terminal_same = 1usize;
    let mut terminal_empty = 0usize;
    let mut terminal_full = 0usize;
    let mut final_frames = Vec::new();
    for _ in 0..300 {
        state = maze_growth_on_tick(state, &mut ctx);
        let frame = maze_growth_render_model(&state).tiles;
        let visible = frame.iter().filter(|tile| **tile).count();
        terminal_same = if frame == previous {
            terminal_same + 1
        } else {
            1
        };
        terminal_empty = if visible == 0 { terminal_empty + 1 } else { 0 };
        terminal_full = if visible == TILE_COUNT {
            terminal_full + 1
        } else {
            0
        };
        assert!(
            state
                .trigger_types
                .iter()
                .filter(|trigger| **trigger == TileTriggerType::Activate)
                .count()
                <= usize::from(state.walker_count) + 2
        );
        final_frames.push(frame.clone());
        if final_frames.len() > 16 {
            final_frames.remove(0);
        }
        previous = frame;
    }
    assert!(terminal_same <= 2);
    assert!(terminal_empty <= 2);
    assert!(terminal_full <= 2);
    assert!(final_frames.windows(2).any(|window| window[0] != window[1]));
}

#[test]
fn walker_normalization_preserves_paths_and_serializes_short_state() {
    let mut tiles = vec![WALL; TILE_COUNT];
    tiles[0] = PATH;
    tiles[1] = WALKER;
    tiles[2] = WALKER;
    tiles[3] = WALKER;
    let state = maze_growth_deserialize(json!({
        "tiles": tiles,
        "visited": [1],
        "ages": ["old", 9],
        "walkers": [1, 1, TILE_COUNT + 9, 2],
        "walkerCount": 2,
        "carvePct": "fast"
    }))
    .unwrap();
    assert_eq!(state.walkers, vec![1, 2]);
    assert_eq!(state.tiles[0], PATH);
    assert_eq!(state.tiles[3], PATH);
    assert_eq!(state.visited.len(), TILE_COUNT);
    assert_eq!(state.ages[0], 0);
    assert_eq!(state.carve_pct, 100);
    let value = maze_growth_serialize(&state).unwrap();
    assert_eq!(value["walkers"], json!([1, 2]));
    assert_eq!(value["tiles"].as_array().unwrap().len(), TILE_COUNT);
}

#[test]
fn collapse_and_exhaustion_preserve_walker_tiles() {
    let mut ctx = context();
    let mut tiles = vec![PATH; TILE_COUNT];
    let ages = (0..TILE_COUNT).map(|i| i as u8).collect::<Vec<_>>();
    tiles[grid_index(7, 7)] = WALKER;
    let state = maze_growth_init(json!({
        "tiles": tiles,
        "ages": ages,
        "walkers": [grid_index(7, 7)],
        "walkerCount": 1,
        "collapseAge": 64,
        "carvePct": 0
    }))
    .unwrap();
    let collapsed = maze_growth_on_input(
        state.clone(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "collapseMaze".into(),
        }),
        &mut ctx,
    );
    assert_eq!(collapsed.tiles[grid_index(7, 7)], WALKER);
    assert_eq!(collapsed.tiles.iter().filter(|c| **c == WALL).count(), 8);

    let renewed = maze_growth_on_tick(state, &mut ctx);
    assert_eq!(renewed.tiles[renewed.walkers[0]], WALKER);
    assert_eq!(renewed.tiles.iter().filter(|c| **c == WALL).count(), 1);
}
