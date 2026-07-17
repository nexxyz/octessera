use super::*;
fn ctx() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_palette_normalize_serialize() {
    let m = physarum_config_menu();
    assert_eq!(m[0].key, "agentCount");
    assert_eq!(m[6].key, "seedSlime");
    let s=physarum_deserialize(serde_json::json!({"x":[999],"y":[-1],"heading":[99],"activeCount":99,"trail":[999],"food":[9],"foodPattern":9,"triggerTypes":["activate"]})).unwrap();
    assert_eq!(s.x[0], WORLD_MAX_X);
    assert_eq!(s.y[0], 0);
    assert_eq!(s.heading[0], 3);
    assert_eq!(s.active_count, MAX_AGENTS);
    assert_eq!(s.trail[0], 255);
    assert_eq!(s.food[0], 1);
    assert_eq!(s.food_pattern, 3);
    assert!(!s.trigger_types.contains(&CellTriggerType::Activate));
    let v = physarum_serialize(&s).unwrap();
    assert!(v.get("triggerTypes").is_none());
    assert!(v.get("tickCounter").is_none());
    assert_eq!(
        physarum_serialize(&physarum_deserialize(v.clone()).unwrap()).unwrap(),
        v
    );
    let model = physarum_render_model(&s);
    assert_eq!(model.name, "physarum");
    assert_eq!(model.palette.active, [240, 255, 140]);
}

#[test]
fn grid_press_food_toggle() {
    let mut c = ctx();
    let s = physarum_init(serde_json::json!({})).unwrap();
    let s = physarum_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.food[grid_index(2, 3)], 1);
    assert_eq!(s.trigger_types[grid_index(2, 3)], CellTriggerType::Stable);
    let s = physarum_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.food[grid_index(2, 3)], 0);
    assert_eq!(
        s.trigger_types[grid_index(2, 3)],
        CellTriggerType::Deactivate
    );
}

#[test]
fn tick_deposits_reflects_and_evaporates() {
    let mut c = ctx();
    let mut s =
        physarum_init(serde_json::json!({"agentCount":1,"depositAmount":64,"evaporationPct":50}))
            .unwrap();
    s.x[0] = 0;
    s.y[0] = 0;
    s.heading[0] = 4;
    s.food.fill(0);
    let t = physarum_on_tick(s, &mut c);
    assert_eq!(t.x[0], 0);
    assert!(t.trail.iter().any(|v| *v > 0));
}

#[test]
fn agent_entry_snapshot_sampling_reflection_and_turn_bias() {
    let mut c = ctx();
    let mut s = physarum_init(serde_json::json!({
        "agentCount":1,"depositAmount":1,"evaporationPct":0
    }))
    .unwrap();
    s.food.fill(0);
    s.trail.fill(0);
    s.x[0] = 0;
    s.y[0] = 0;
    s.heading[0] = 0;
    let t = physarum_on_tick(s, &mut c);
    assert_eq!(t.trigger_types[grid_index(1, 0)], CellTriggerType::Activate);

    let mut s = physarum_init(serde_json::json!({
        "agentCount":2,"depositAmount":64,"evaporationPct":0,"turnBiasPct":100
    }))
    .unwrap();
    s.food.fill(0);
    s.trail.fill(0);
    s.x[0] = 3 * UNIT;
    s.y[0] = 2 * UNIT;
    s.heading[0] = 0;
    s.x[1] = 3 * UNIT;
    s.y[1] = UNIT;
    s.heading[1] = 0;
    let t = physarum_on_tick(s, &mut c);
    assert_eq!(t.heading[1], 0);

    let mut s = physarum_init(serde_json::json!({"agentCount":1})).unwrap();
    s.food.fill(0);
    s.trail.fill(0);
    s.x[0] = WORLD_MAX_X;
    s.y[0] = UNIT;
    s.heading[0] = 1;
    let t = physarum_on_tick(s, &mut c);
    assert_eq!(t.heading[0], 3);

    let mut s = physarum_init(serde_json::json!({"agentCount":1})).unwrap();
    s.food.fill(0);
    s.trail.fill(0);
    s.x[0] = UNIT;
    s.y[0] = WORLD_MAX_Y;
    s.heading[0] = 1;
    let t = physarum_on_tick(s, &mut c);
    assert_eq!(t.heading[0], 7);

    let mut s = physarum_init(serde_json::json!({
        "agentCount":1,"turnBiasPct":0,"senseDistance":1
    }))
    .unwrap();
    s.food.fill(0);
    s.trail.fill(0);
    s.x[0] = 3 * UNIT;
    s.y[0] = 3 * UNIT;
    s.heading[0] = 0;
    s.trail[grid_index(4, 4)] = 1;
    let no_bias = physarum_on_tick(s.clone(), &mut c);
    s.turn_bias_pct = 100;
    let with_bias = physarum_on_tick(s, &mut c);
    assert_eq!(no_bias.heading[0], 0);
    assert_eq!(with_bias.heading[0], 1);
}

#[test]
fn seed_and_relocate_actions() {
    let mut c = ctx();
    let mut s = physarum_init(serde_json::json!({"agentCount":2})).unwrap();
    s.trail[0] = 100;
    s.food.fill(0);
    let seeded = physarum_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedSlime".into(),
        }),
        &mut c,
    );
    assert_eq!(seeded.trigger_types[0], CellTriggerType::Deactivate);
    assert!(seeded.trigger_types.contains(&CellTriggerType::Activate));
    let relocated = physarum_on_input(
        seeded,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "relocateFood".into(),
        }),
        &mut c,
    );
    assert!(!relocated.trigger_types.contains(&CellTriggerType::Activate));
    assert_eq!(relocated.food_pattern, 1);
}
