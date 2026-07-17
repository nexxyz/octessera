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
    assert_mixed(&s);
    let start_region = s.region_index;
    let start_zoom = s.zoom;
    let ticked = fractal_explorer_on_tick(s, &mut ctx());
    assert_eq!(ticked.region_index, start_region);
    assert!(ticked.zoom > start_zoom);
    assert_mixed(&ticked);
}

#[test]
fn default_viewport_remains_mixed_and_evolves_over_many_ticks() {
    let mut c = ctx();
    let mut s = fractal_explorer_init(serde_json::json!({})).unwrap();
    let mut states = Vec::new();
    for _ in 0..192 {
        s = fractal_explorer_on_tick(s, &mut c);
        assert_mixed(&s);
        let state = (s.center_x, s.center_y, s.zoom, s.classes.clone());
        if !states.contains(&state) {
            states.push(state);
        }
    }
    assert!(states.len() > 16);
}

#[test]
fn zero_zoom_rate_holds_zoom_while_steering() {
    let mut c = ctx();
    let mut s = fractal_explorer_init(serde_json::json!({ "zoomRatePct": 0 })).unwrap();
    let zoom = s.zoom;
    for _ in 0..12 {
        s = fractal_explorer_on_tick(s, &mut c);
        assert_eq!(s.zoom, zoom);
        assert_mixed(&s);
    }
}

#[test]
fn zero_zoom_and_zero_drift_hold_healthy_viewport_steady() {
    let mut c = ctx();
    let mut s = fractal_explorer_init(serde_json::json!({
        "zoomRatePct": 0,
        "driftPct": 0
    }))
    .unwrap();
    let steady = (s.center_x, s.center_y, s.zoom, s.classes.clone());
    for _ in 0..8 {
        s = fractal_explorer_on_tick(s, &mut c);
        assert_eq!((s.center_x, s.center_y, s.zoom, s.classes.clone()), steady);
        assert_mixed(&s);
    }
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
fn bad_viewport_recovers_and_bounds_fallback_triggers() {
    let mut c = ctx();
    let s = fractal_explorer_init(
        serde_json::json!({"centerX":4096,"centerY":4096,"zoom":16000,"zoomRatePct":100}),
    )
    .unwrap();
    let t = fractal_explorer_on_tick(s, &mut c);
    assert_mixed(&t);
    let activations = t
        .trigger_types
        .iter()
        .filter(|trigger| **trigger == CellTriggerType::Activate)
        .count();
    assert!(activations <= 8);
}

#[test]
fn manual_jump_and_toggle_keep_valid_classes() {
    let mut c = ctx();
    let s = fractal_explorer_init(serde_json::json!({})).unwrap();
    let jumped = fractal_explorer_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "jumpRegion".into(),
        }),
        &mut c,
    );
    assert_mixed(&jumped);
    let toggled = fractal_explorer_on_input(
        jumped,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "toggleFractalMode".into(),
        }),
        &mut c,
    );
    assert_eq!(toggled.mode, "julia");
    assert_mixed(&toggled);
    let mut s = toggled;
    for _ in 0..8 {
        s = fractal_explorer_on_tick(s, &mut c);
        assert_eq!(s.mode, "julia");
        assert_mixed(&s);
    }
}

#[test]
fn drift_bias_breaks_close_ties_without_overwhelming_score() {
    let mut classes = vec![0; CELL_COUNT];
    for y in 0..GRID_HEIGHT {
        for x in 4..GRID_WIDTH {
            classes[grid_index(x, y)] = 2;
        }
    }
    let mut s = fractal_explorer_init(serde_json::json!({ "driftX": 3, "driftY": 0 })).unwrap();
    let east = candidate_score(&classes, 1, 0, &s);
    let west = candidate_score(&classes, -1, 0, &s);
    assert!(east > west);
    assert!(east - west <= 2);

    s.drift_pct = 0;
    assert_eq!(
        candidate_score(&classes, 1, 0, &s),
        candidate_score(&classes, -1, 0, &s)
    );
}

fn assert_mixed(s: &FractalExplorerState) {
    let visible = s.classes.iter().filter(|class| **class != 0).count();
    assert!((4..=CELL_COUNT - 4).contains(&visible));
    assert!(edge_transitions(&s.classes) > 0);
}
