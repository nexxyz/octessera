use super::*;

fn ctx() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_palette_normalize_serialize() {
    let m = orbit_config_menu();
    assert_eq!(m[0].key, "particleCount");
    assert_eq!(m[0].max, Some(16));
    assert_eq!(m[3].key, "repelMode");
    assert_eq!(m[4].key, "resetOrbit");
    assert_eq!(m[5].key, "nudgeAttractor");
    let s=orbit_deserialize(serde_json::json!({"x":[999],"y":[-1],"vx":[99],"vy":[-99],"activeCount":99,"attractorX":999,"attractorY":-1,"repelMode":"bad","tickCounter":9,"triggerTypes":["activate"]})).unwrap();
    assert_eq!(s.x[0], WORLD_MAX_X);
    assert_eq!(s.y[0], 0);
    assert_eq!(s.vx[0], SPEED);
    assert_eq!(s.vy[0], -SPEED);
    assert_eq!(s.active_count, MAX_PARTICLES);
    assert_eq!(s.repel_mode, "off");
    assert_eq!(s.tick_counter, 0);
    assert!(!s.trigger_types.contains(&CellTriggerType::Activate));
    let v = orbit_serialize(&s).unwrap();
    assert!(v.get("triggerTypes").is_none());
    assert!(v.get("tickCounter").is_none());
    assert_eq!(
        orbit_serialize(&orbit_deserialize(v.clone()).unwrap()).unwrap(),
        v
    );
    let model = orbit_render_model(&s);
    assert_eq!(model.name, "orbit");
    assert_eq!(model.palette.active, [255, 210, 120]);
    assert_eq!(model.palette.inactive, crate::palette::BLACK);

    let initialized = orbit_init(serde_json::json!({})).unwrap();
    assert!(!initialized
        .trigger_types
        .contains(&CellTriggerType::Activate));
}

#[test]
fn grid_press_forces_attractor_activate() {
    let mut c = ctx();
    let s = orbit_init(serde_json::json!({"particleCount":4})).unwrap();
    let s = orbit_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.attractor_x, 32);
    assert_eq!(s.attractor_y, 48);
    assert_eq!(s.trigger_types[grid_index(2, 3)], CellTriggerType::Activate);
}

#[test]
fn tick_reflects_and_orbit_changes_velocity() {
    let mut c = ctx();
    let s=orbit_init(serde_json::json!({"particleCount":1,"x":[0],"y":[16],"vx":[-14],"vy":[0],"attractorX":64,"attractorY":64,"attractionPct":100,"orbitPct":100})).unwrap();
    let t = orbit_on_tick(s, &mut c);
    assert_eq!(t.x[0], 0);
    assert!(t.vx[0] >= 0);
    assert_ne!(t.vy[0], 0);
}

#[test]
fn reset_and_nudge_are_deterministic() {
    let mut c = ctx();
    let s = orbit_init(serde_json::json!({"particleCount":2})).unwrap();
    let reset = orbit_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "resetOrbit".into(),
        }),
        &mut c,
    );
    assert_eq!(reset.attractor_x, 64);
    assert!(reset.trigger_types.contains(&CellTriggerType::Activate));
    let nudge = orbit_on_input(
        reset.clone(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "nudgeAttractor".into(),
        }),
        &mut c,
    );
    assert_eq!(nudge.attractor_vx, reset.attractor_vx + 2);
    assert!(!nudge.trigger_types.contains(&CellTriggerType::Activate));
}
