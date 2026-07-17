use super::*;
fn ctx() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_palette_normalize_serialize() {
    let m = fractal_explorer_config_menu();
    assert_eq!(m[0].key, "zoomRatePct");
    assert_eq!(m[3].key, "fractalMode");
    assert_eq!(m[4].key, "jumpRegion");
    let s=fractal_explorer_deserialize(serde_json::json!({"centerX":99999,"zoom":1,"driftX":999,"juliaCx":99999,"mode":"bad","regionIndex":99,"classes":[9],"triggerTypes":["activate"],"iterationLimit":99})).unwrap();
    assert_eq!(s.center_x, 4096);
    assert_eq!(s.zoom, 4);
    assert_eq!(s.drift_x, 64);
    assert_eq!(s.julia_cx, 2048);
    assert_eq!(s.mode, "mandelbrot");
    assert_eq!(s.region_index, 7);
    assert_eq!(s.classes[0], 2);
    assert!(!s.trigger_types.contains(&CellTriggerType::Activate));
    let v = fractal_explorer_serialize(&s).unwrap();
    assert!(v.get("triggerTypes").is_none());
    assert!(v.get("tickCounter").is_none());
    assert_eq!(
        fractal_explorer_serialize(&fractal_explorer_deserialize(v.clone()).unwrap()).unwrap(),
        v
    );
    let model = fractal_explorer_render_model(&s);
    assert_eq!(model.name, "fractal explorer");
    assert_eq!(model.palette.active, [255, 220, 180]);
    assert!(model.status_line.contains("Z:4"));
}

#[test]
fn default_viewport_has_mixed_mandelbrot_classes() {
    let s = fractal_explorer_init(serde_json::json!({})).unwrap();
    assert!(s.classes.contains(&0));
    assert!(s.classes.contains(&1));
    assert!(s.classes.contains(&2));
    let start_region = s.region_index;
    let start_zoom = s.zoom;
    let ticked = fractal_explorer_on_tick(s, &mut ctx());
    assert_eq!(ticked.region_index, start_region);
    assert!(ticked.zoom > start_zoom);
    assert!(ticked.classes.contains(&0));
    assert!(ticked.classes.contains(&1));
    assert!(ticked.classes.contains(&2));
}

#[test]
fn grid_press_forces_only_pressed_cell() {
    let mut c = ctx();
    let s = fractal_explorer_init(serde_json::json!({})).unwrap();
    let pressed = fractal_explorer_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(
        pressed.trigger_types[grid_index(2, 3)],
        CellTriggerType::Activate
    );
}

#[test]
fn tick_jump_toggle_and_class_triggers() {
    let mut c = ctx();
    let s = fractal_explorer_init(serde_json::json!({"zoom":16000,"zoomRatePct":100})).unwrap();
    let old = s.region_index;
    let t = fractal_explorer_on_tick(s, &mut c);
    assert_ne!(t.region_index, old);
    let jumped = fractal_explorer_on_input(
        t,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "jumpRegion".into(),
        }),
        &mut c,
    );
    let toggled = fractal_explorer_on_input(
        jumped,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "toggleFractalMode".into(),
        }),
        &mut c,
    );
    assert_eq!(toggled.mode, "julia");
    assert_eq!(toggled.classes.len(), CELL_COUNT);
}
