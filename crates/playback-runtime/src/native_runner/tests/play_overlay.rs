use super::*;

#[test]
pub(crate) fn fn_overlay_shows_active_layers_and_play_page_options() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "pan".into();
    runner.play_mode = "pan".into();
    runner.layer_behavior_ids[1] = "none".into();
    runner.layer_behavior_ids[2] = "life".into();
    runner.display.ui.fn_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let active_layer = &cells[display_index(0, 0)];
    let none_layer = &cells[display_index(0, 1)];
    let configured_layer = &cells[display_index(0, 2)];
    let selected_page = &cells[display_index(GRID_WIDTH - 1, 1)];
    let middle_cell = cells[display_index(3, 3)].as_object().unwrap();

    assert_eq!(*active_layer, led_rgb(platform_core::palette::BLUE));
    assert_eq!(
        *none_layer,
        led_rgb(dim_rgb(platform_core::palette::GRAY, 8))
    );
    assert_eq!(configured_layer, active_layer);
    assert_eq!(*selected_page, led_rgb(platform_core::palette::GREEN));
    assert_eq!(middle_cell["r"], 0);
    assert_eq!(middle_cell["g"], 0);
    assert_eq!(middle_cell["b"], 0);
}

#[test]
pub(crate) fn fn_overlay_highlights_active_layer_when_not_in_play_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "none".into();
    runner.play_mode = "mix".into();
    runner.layer_behavior_ids[1] = "none".into();
    runner.layer_behavior_ids[2] = "life".into();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 3, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    runner.display.ui.fn_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let active_layer = &cells[display_index(0, 0)];
    let none_layer = &cells[display_index(0, 1)];
    let configured_layer = &cells[display_index(0, 2)];
    let play_page = &cells[display_index(GRID_WIDTH - 1, 0)];
    let middle_cell = cells[display_index(3, 3)].as_object().unwrap();

    assert_eq!(*active_layer, led_rgb(platform_core::palette::BLUE));
    assert_eq!(
        *none_layer,
        led_rgb(dim_rgb(platform_core::palette::GRAY, 8))
    );
    assert_eq!(*configured_layer, led_rgb(platform_core::palette::GREEN));
    assert_eq!(
        *play_page,
        led_rgb(dim_rgb(platform_core::palette::YELLOW, 4))
    );
    assert_eq!(middle_cell["r"], 0);
    assert_eq!(middle_cell["g"], 0);
    assert_eq!(middle_cell["b"], 0);
}

#[test]
pub(crate) fn combined_modifier_overlay_shows_layer_column_only() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "pan".into();
    runner.play_mode = "pan".into();
    runner.display.ui.combined_modifier_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let active_layer = &cells[display_index(0, 0)];
    let right_page = cells[display_index(GRID_WIDTH - 1, 1)].as_object().unwrap();

    assert_eq!(*active_layer, led_rgb(platform_core::palette::BLUE));
    assert_eq!(right_page["r"], 0);
    assert_eq!(right_page["g"], 0);
    assert_eq!(right_page["b"], 0);
}

#[test]
pub(crate) fn fn_overlay_dims_fx_grid_cells_when_play_mode_is_fx() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.ui.fn_held = true;
    runner.active_play_mode = "fx".into();
    runner.play_mode = "fx".into();

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let mid_cell = cells[display_index(2, 2)].as_object().unwrap();
    let fx_page = &cells[display_index(GRID_WIDTH - 1, 2)];
    let layer_cell = cells[display_index(0, 0)].as_object().unwrap();

    assert!(mid_cell["r"].as_i64().unwrap() < 20);
    assert!(mid_cell["g"].as_i64().unwrap() < 20);
    assert!(mid_cell["b"].as_i64().unwrap() < 20);
    assert_eq!(*fx_page, led_rgb(platform_core::palette::GREEN));
    assert!(layer_cell["g"].as_i64().unwrap() > 0);
}

#[test]
pub(crate) fn assignment_overlays_suppress_fn_navigation_overlay() {
    for guard in ["sample", "trigger", "play"] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.display.ui.fn_held = true;
        match guard {
            "sample" => runner.sample_assign = Some((0, 0)),
            "trigger" => runner.trigger_probability_assign = Some(0),
            _ => runner.play_fx_assign = Some(json!({ "fxType": "filter" })),
        }

        let snapshot = runner.snapshot().unwrap();
        let cells = led_cells(&snapshot);

        assert_ne!(
            cells[display_index(0, 0)],
            led_rgb(platform_core::palette::BLUE)
        );
        assert_ne!(
            cells[display_index(GRID_WIDTH - 1, 0)],
            led_rgb(dim_rgb(platform_core::palette::YELLOW, 4))
        );
    }
}

#[test]
pub(crate) fn play_transpose_overlay_distinguishes_selected_center_and_potential_keys() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "transpose".into();
    runner.play_transpose_offsets[0] = 7;
    runner.play_transpose_selected.fill(false);
    runner.play_transpose_selected[0] = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let center = &cells[display_index(1, 3)];
    let selected = &cells[display_index(5, 3)];
    let potential = &cells[display_index(2, 3)];

    assert_eq!(*center, led_rgb(platform_core::palette::WHITE));
    assert_eq!(*selected, led_rgb(platform_core::palette::GREEN));
    assert_eq!(
        *potential,
        led_rgb(dim_rgb(platform_core::palette::BLUE, 3))
    );
}
