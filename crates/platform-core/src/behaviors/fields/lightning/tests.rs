use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}
fn base(edge: &str) -> LightningState {
    lightning_init(serde_json::json!({"branchChancePct":0,"jitterChancePct":0,"decayTicks":2,"leaderLimit":3,"targetEdge":edge})).unwrap()
}

#[test]
fn menu_palette_and_normalization_contract() {
    let menu = lightning_config_menu();
    assert_eq!(menu[0].key, "branchChancePct");
    assert_eq!(menu[0].max, Some(100));
    assert_eq!(menu[1].key, "jitterChancePct");
    assert_eq!(menu[2].key, "decayTicks");
    assert_eq!(menu[2].max, Some(16));
    assert_eq!(menu[3].key, "leaderLimit");
    assert_eq!(
        menu[4].options.as_ref().unwrap(),
        &vec!["north", "east", "south", "west"]
    );
    assert_eq!(menu[5].key, "strikeNow");
    let state = lightning_deserialize(serde_json::json!({"cells":[9,1],"ages":[999],"leaders":[{"x":99,"y":0}],"branchChancePct":999,"jitterChancePct":999,"decayTicks":99,"leaderLimit":99,"targetEdge":"bad"})).unwrap();
    assert_eq!(state.cells[0], FLASH);
    assert_eq!(state.target_edge, "south");
    assert_eq!(state.decay_ticks, 16);
    assert_eq!(state.leader_limit, 8);
    assert!(!state.trigger_types.contains(&CellTriggerType::Activate));
    let model = lightning_render_model(&state);
    assert_eq!(model.name, "lightning");
    assert_eq!(model.palette.inactive, crate::palette::BLACK);
    assert_eq!(model.palette.active, [255, 255, 180]);

    let restored_orphan = lightning_deserialize(serde_json::json!({
        "cells":[1],
        "leaders":[],
        "targetEdge":"south"
    }))
    .unwrap();
    assert_eq!(restored_orphan.leaders, vec![LeaderPos { x: 0, y: 0 }]);
}

#[test]
fn target_edges_seed_opposite_and_move_without_cascade() {
    let mut context = context();
    let south = base("south");
    assert!(south.leaders.iter().all(|p| p.y == GRID_HEIGHT - 1));
    let north = base("north");
    assert!(north.leaders.iter().all(|p| p.y == 0));
    let east = base("east");
    assert!(east.leaders.iter().all(|p| p.x == 0));
    let west = base("west");
    assert!(west.leaders.iter().all(|p| p.x == GRID_WIDTH - 1));
    let mut state = base("south");
    clear(&mut state);
    add_leader(&mut state, 3, 6);
    let ticked = lightning_on_tick(state, &mut context);
    assert!(ticked.cells[grid_index(3, 5)] != EMPTY);
    assert_eq!(ticked.cells[grid_index(3, 4)], EMPTY);
}

#[test]
fn flash_reactivates_then_stable_then_clear_deactivate_and_restart() {
    let mut context = context();
    let mut state = base("south");
    clear(&mut state);
    add_leader(&mut state, 2, 1);
    state.cells[grid_index(2, 2)] = LEADER;
    let flashed = lightning_on_tick(state, &mut context);
    assert_eq!(flashed.cells[grid_index(2, 1)], FLASH);
    assert_eq!(
        flashed.trigger_types[grid_index(2, 2)],
        CellTriggerType::Activate
    );
    let stable = lightning_on_tick(flashed, &mut context);
    assert_eq!(
        stable.trigger_types[grid_index(2, 1)],
        CellTriggerType::Stable
    );
    let restarted = lightning_on_tick(stable, &mut context);
    assert_eq!(
        restarted.trigger_types[grid_index(2, 1)],
        CellTriggerType::Deactivate
    );
    assert!(!restarted.leaders.is_empty());
}

#[test]
fn grid_press_strike_now_and_serialization_are_stable() {
    let mut context = context();
    let state = base("east");
    let pressed = lightning_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_eq!(pressed.leaders, vec![LeaderPos { x: 2, y: 3 }]);
    assert_eq!(
        pressed.trigger_types[grid_index(2, 3)],
        CellTriggerType::Activate
    );
    let struck = lightning_on_input(
        pressed,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "strikeNow".into(),
        }),
        &mut context,
    );
    assert_eq!(struck.leaders.len(), 1);
    assert_eq!(struck.leaders[0].x, 0);
    let serialized = lightning_serialize(&struck).unwrap();
    assert!(serialized.get("triggerTypes").is_none());
    let restored = lightning_deserialize(serialized.clone()).unwrap();
    assert_eq!(lightning_serialize(&restored).unwrap(), serialized);
}
