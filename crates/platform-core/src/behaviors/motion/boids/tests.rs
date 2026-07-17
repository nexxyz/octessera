use super::*;

fn ctx() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_palette_normalize_serialize() {
    let m = boids_config_menu();
    assert_eq!(m[0].key, "flockSize");
    assert_eq!(m[0].max, Some(24));
    assert_eq!(m[4].key, "scatterFlock");
    assert_eq!(m[5].key, "seedFlock");
    let s=boids_deserialize(serde_json::json!({"x":[999],"y":[-9],"vx":[99],"vy":[-99],"activeCount":99,"separationPct":999,"alignmentPct":999,"cohesionPct":999,"tickCounter":7,"triggerTypes":["activate"]})).unwrap();
    assert_eq!(s.x.len(), MAX_BOIDS);
    assert_eq!(s.x[0], WORLD_MAX_X);
    assert_eq!(s.y[0], 0);
    assert_eq!(s.vx[0], SPEED);
    assert_eq!(s.vy[0], -SPEED);
    assert_eq!(s.active_count, MAX_BOIDS);
    assert_eq!(s.tick_counter, 0);
    assert!(!s.trigger_types.contains(&CellTriggerType::Activate));
    let v = boids_serialize(&s).unwrap();
    assert!(v.get("triggerTypes").is_none());
    assert!(v.get("tickCounter").is_none());
    assert_eq!(
        boids_serialize(&boids_deserialize(v.clone()).unwrap()).unwrap(),
        v
    );
    let model = boids_render_model(&s);
    assert_eq!(model.name, "boids");
    assert_eq!(model.palette.active, [255, 240, 160]);
    assert_eq!(model.palette.inactive, crate::palette::BLACK);
}

#[test]
fn grid_press_world_space_and_max_replacement() {
    let mut c = ctx();
    let mut s = boids_init(serde_json::json!({"flockSize":24})).unwrap();
    s = boids_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert!(s
        .x
        .iter()
        .zip(s.y.iter())
        .take(s.active_count)
        .any(|(x, y)| *x == 32 && *y == 48));
    assert_eq!(s.trigger_types[grid_index(2, 3)], CellTriggerType::Activate);

    let occupied = boids_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(
        occupied.trigger_types[grid_index(2, 3)],
        CellTriggerType::Activate
    );
}

#[test]
fn tick_reflects_socially_changes_and_trigger_contract() {
    let mut c = ctx();
    let s=boids_init(serde_json::json!({"flockSize":2,"x":[0,16],"y":[0,0],"vx":[-12,0],"vy":[0,0],"separationPct":100,"alignmentPct":100,"cohesionPct":100})).unwrap();
    let before = s.vx.clone();
    let t = boids_on_tick(s, &mut c);
    assert_eq!(t.x[0], 0);
    assert!(t.vx[0] >= 0);
    assert_ne!(t.vx, before);
    assert!(t
        .trigger_types
        .iter()
        .any(|v| *v == CellTriggerType::Activate || *v == CellTriggerType::Stable));
}

#[test]
fn scatter_velocity_only_and_seed_activates() {
    let mut c = ctx();
    let s = boids_init(serde_json::json!({"flockSize":3})).unwrap();
    let pos = (s.x.clone(), s.y.clone());
    let scattered = boids_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "scatterFlock".into(),
        }),
        &mut c,
    );
    assert_eq!((scattered.x.clone(), scattered.y.clone()), pos);
    assert!(!scattered.trigger_types.contains(&CellTriggerType::Activate));
    let mut empty =
        boids_deserialize(serde_json::json!({"activeCount":1,"x":[0],"y":[0]})).unwrap();
    empty.x[0] = 100;
    empty.y[0] = 100;
    let seeded = boids_on_input(
        empty,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedFlock".into(),
        }),
        &mut c,
    );
    assert_eq!(seeded.x[0], 48);
    assert!(seeded.trigger_types.contains(&CellTriggerType::Activate));
}
