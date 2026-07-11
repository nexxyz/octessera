use super::*;

#[test]
pub(crate) fn fn_overlay_shows_active_layers_and_sparks_page_options() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "pan".into();
    runner.sparks_mode = "pan".into();
    runner.layer_behavior_ids[1] = "none".into();
    runner.layer_behavior_ids[2] = "life".into();
    runner.ui.fn_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let active_layer = cells[display_index(0, 0)].as_object().unwrap();
    let none_layer = cells[display_index(0, 1)].as_object().unwrap();
    let configured_layer = cells[display_index(0, 2)].as_object().unwrap();
    let selected_page = cells[display_index(GRID_WIDTH - 1, 1)].as_object().unwrap();
    let middle_cell = cells[display_index(3, 3)].as_object().unwrap();

    assert_eq!(active_layer["r"].as_i64().unwrap(), 0);
    assert_eq!(active_layer["g"].as_i64().unwrap(), 120);
    assert_eq!(active_layer["b"].as_i64().unwrap(), 0);
    assert_eq!(none_layer["r"].as_i64().unwrap(), 0);
    assert_eq!(none_layer["g"].as_i64().unwrap(), 48);
    assert_eq!(none_layer["b"].as_i64().unwrap(), 23);
    assert_eq!(configured_layer, active_layer);
    assert!(selected_page["g"].as_i64().unwrap() > 0 || selected_page["b"].as_i64().unwrap() > 0);
    assert!(middle_cell["r"].as_i64().unwrap() < 70);
    assert!(middle_cell["g"].as_i64().unwrap() < 70);
    assert!(middle_cell["b"].as_i64().unwrap() < 70);
}

#[test]
pub(crate) fn fn_overlay_highlights_active_layer_when_not_in_sparks_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "none".into();
    runner.sparks_mode = "mix".into();
    runner.layer_behavior_ids[1] = "none".into();
    runner.layer_behavior_ids[2] = "life".into();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 3, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    runner.ui.fn_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let active_layer = cells[display_index(0, 0)].as_object().unwrap();
    let none_layer = cells[display_index(0, 1)].as_object().unwrap();
    let configured_layer = cells[display_index(0, 2)].as_object().unwrap();
    let sparks_page = cells[display_index(GRID_WIDTH - 1, 0)].as_object().unwrap();
    let middle_cell = cells[display_index(3, 3)].as_object().unwrap();

    assert_eq!(active_layer["r"].as_i64().unwrap(), 0);
    assert_eq!(active_layer["g"].as_i64().unwrap(), 191);
    assert_eq!(active_layer["b"].as_i64().unwrap(), 191);
    assert_eq!(none_layer["r"].as_i64().unwrap(), 0);
    assert_eq!(none_layer["g"].as_i64().unwrap(), 48);
    assert_eq!(none_layer["b"].as_i64().unwrap(), 23);
    assert_eq!(configured_layer["r"].as_i64().unwrap(), 0);
    assert_eq!(configured_layer["g"].as_i64().unwrap(), 120);
    assert_eq!(configured_layer["b"].as_i64().unwrap(), 0);
    assert_eq!(sparks_page["r"].as_i64().unwrap(), 0);
    assert_eq!(sparks_page["g"].as_i64().unwrap(), 60);
    assert_eq!(sparks_page["b"].as_i64().unwrap(), 60);
    assert!(middle_cell["r"].as_i64().unwrap() < 70);
    assert!(middle_cell["g"].as_i64().unwrap() < 70);
    assert!(middle_cell["b"].as_i64().unwrap() < 70);
}

#[test]
pub(crate) fn fn_overlay_dims_fx_grid_cells_when_sparks_mode_is_fx() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.ui.fn_held = true;
    runner.active_sparks_mode = "fx".into();
    runner.sparks_mode = "fx".into();

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let mid_cell = cells[display_index(2, 2)].as_object().unwrap();
    let fx_page = cells[display_index(GRID_WIDTH - 1, 2)].as_object().unwrap();
    let layer_cell = cells[display_index(0, 0)].as_object().unwrap();

    assert!(mid_cell["r"].as_i64().unwrap() < 20);
    assert!(mid_cell["g"].as_i64().unwrap() < 20);
    assert!(mid_cell["b"].as_i64().unwrap() < 20);
    assert!(fx_page["g"].as_i64().unwrap() > 100 && fx_page["g"].as_i64().unwrap() < 200);
    assert!(fx_page["g"].as_i64().unwrap() > 0 || fx_page["b"].as_i64().unwrap() > 0);
    assert!(layer_cell["g"].as_i64().unwrap() > 0);
}
