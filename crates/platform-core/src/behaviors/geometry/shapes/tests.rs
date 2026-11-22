use super::*;

#[test]
fn pulse_expands_lifespans_and_wavefront_only_adds_leading_edge() {
    let mut context = BehaviorContext::new(120.0);
    let state = shapes_init(
        serde_json::json!({ "pulseShape": "diamond", "lifespan": 2, "maxRadius": 4, "autoPulseInterval": 0 }),
    )
    .unwrap();
    let state = shapes_on_input(state, DeviceInput::GridPress { x: 3, y: 3 }, &mut context);
    assert_eq!(state.pulses[0].radius, 0);
    assert!(state.lifetimes[grid_index(3, 3)] > 0);

    let ticked = shapes_on_tick(state, &mut context);
    assert_eq!(ticked.pulses[0].radius, 1);
    assert!(ticked.lifetimes[grid_index(4, 3)] > 0);

    let mut ticked = ticked;
    for radius in 2..=4 {
        ticked = shapes_on_tick(ticked, &mut context);
        assert_eq!(ticked.pulses[0].radius, radius);
    }
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

#[test]
fn a_filled_shape_lights_its_whole_disk_while_it_grows() {
    let mut context = BehaviorContext::new(120.0);
    let config = serde_json::json!({ "lifespan": 1, "maxRadius": 4, "autoPulseInterval": 0 });
    for (shape, center_lit) in [("filled", true), ("ring", false)] {
        let mut config = config.clone();
        config["pulseShape"] = shape.into();
        let mut state = shapes_init(config).unwrap();
        for _ in 0..2 {
            state = shapes_on_tick(state, &mut context);
        }
        assert_eq!(state.pulses[0].radius, 2);
        assert_eq!(state.lifetimes[grid_index(4, 4)] > 0, center_lit, "{shape}");
        assert!(state.lifetimes[grid_index(6, 4)] > 0, "{shape}");
    }
}

#[test]
fn a_loaded_max_radius_is_held_to_4_through_12() {
    for (loaded, held) in [(2, 4), (32, 12)] {
        let state = shapes_init(serde_json::json!({ "maxRadius": loaded })).unwrap();
        assert_eq!((state.max_radius, state.pulses[0].max_radius), (held, held));
        let mut saved = serde_json::to_value(&state).unwrap();
        saved["maxRadius"] = loaded.into();
        saved["pulses"][0]["maxRadius"] = loaded.into();
        let state = shapes_deserialize(saved).unwrap();
        assert_eq!((state.max_radius, state.pulses[0].max_radius), (held, held));
    }
}
