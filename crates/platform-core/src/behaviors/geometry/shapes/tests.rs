use super::*;

#[test]
fn pulse_expands_lifespans_and_wavefront_only_adds_leading_edge() {
    let mut context = BehaviorContext::new(120.0);
    let state = shapes_init(
        serde_json::json!({ "pulseShape": "diamond", "lifespan": 2, "maxRadius": 2, "autoPulseInterval": 0 }),
    )
    .unwrap();
    let state = shapes_on_input(state, DeviceInput::GridPress { x: 3, y: 3 }, &mut context);
    assert_eq!(state.pulses[0].radius, 0);
    assert!(state.lifetimes[grid_index(3, 3)] > 0);

    let ticked = shapes_on_tick(state, &mut context);
    assert_eq!(ticked.pulses[0].radius, 1);
    assert!(ticked.lifetimes[grid_index(4, 3)] > 0);

    let ticked = shapes_on_tick(ticked, &mut context);
    assert_eq!(ticked.pulses[0].radius, 2);
    let expired = shapes_on_tick(ticked, &mut context);
    assert!(expired.pulses.is_empty());
}

#[test]
fn shapes_render_and_config_menu_match_contract() {
    let state = shapes_init(Value::Null).unwrap();
    let model = shapes_render_model(&state);
    assert_eq!(model.name, "shapes");
    assert!(model.status_line.contains("ring"));
    assert_eq!(model.trigger_types.as_ref().unwrap().len(), TILE_COUNT);
    assert_eq!(
        shapes_config_menu()
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec![
            "pulseShape",
            "lifespan",
            "maxRadius",
            "autoPulseInterval",
            "spawnStep",
            "spawnPulse"
        ]
    );
}

#[test]
fn default_avoids_consecutive_empty_or_full_frames() {
    let mut context = BehaviorContext::new(120.0);
    let mut state = shapes_init(Value::Null).unwrap();
    let mut empty_run = 0usize;
    let mut full_run = 0usize;
    for _ in 0..300 {
        state = shapes_on_tick(state, &mut context);
        let tiles = shapes_render_model(&state).tiles;
        let visible = tiles.iter().filter(|tile| **tile).count();
        empty_run = if visible == 0 { empty_run + 1 } else { 0 };
        full_run = if visible == TILE_COUNT {
            full_run + 1
        } else {
            0
        };
        assert!(empty_run <= 1);
        assert!(full_run <= 1);
        assert!(
            state
                .trigger_types
                .iter()
                .filter(|trigger| **trigger == TileTriggerType::Activate)
                .count()
                <= TILE_COUNT / 2
        );
    }
}
