use super::*;

#[test]
fn fn_overlay_shows_active_parts_and_dance_page_options() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "pan".into();
    runner.dance_mode = "pan".into();
    runner.part_behavior_ids[1] = "none".into();
    runner.part_behavior_ids[2] = "life".into();
    runner.ui.fn_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let active_part = cells[display_index(0, 0)].as_object().unwrap();
    let none_part = cells[display_index(0, 1)].as_object().unwrap();
    let configured_part = cells[display_index(0, 2)].as_object().unwrap();
    let selected_page = cells[display_index(GRID_WIDTH - 1, 1)].as_object().unwrap();
    let middle_cell = cells[display_index(3, 3)].as_object().unwrap();

    assert_eq!(active_part["r"].as_i64().unwrap(), 0);
    assert_eq!(active_part["g"].as_i64().unwrap(), 120);
    assert_eq!(active_part["b"].as_i64().unwrap(), 0);
    assert_eq!(none_part["r"].as_i64().unwrap(), 0);
    assert_eq!(none_part["g"].as_i64().unwrap(), 48);
    assert_eq!(none_part["b"].as_i64().unwrap(), 23);
    assert_eq!(configured_part, active_part);
    assert!(selected_page["g"].as_i64().unwrap() > 0 || selected_page["b"].as_i64().unwrap() > 0);
    assert!(middle_cell["r"].as_i64().unwrap() < 70);
    assert!(middle_cell["g"].as_i64().unwrap() < 70);
    assert!(middle_cell["b"].as_i64().unwrap() < 70);
}

#[test]
fn fn_overlay_highlights_active_part_when_not_in_dance_mode() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_dance_mode = "none".into();
    runner.dance_mode = "mix".into();
    runner.part_behavior_ids[1] = "none".into();
    runner.part_behavior_ids[2] = "life".into();
    runner.ui.fn_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let active_part = cells[display_index(0, 0)].as_object().unwrap();
    let none_part = cells[display_index(0, 1)].as_object().unwrap();
    let configured_part = cells[display_index(0, 2)].as_object().unwrap();
    let dance_page = cells[display_index(GRID_WIDTH - 1, 0)].as_object().unwrap();

    assert!(active_part["g"].as_i64().unwrap() > 0);
    assert!(active_part["b"].as_i64().unwrap() > 0);
    assert_eq!(active_part["g"], active_part["b"]);
    assert!(active_part["r"].as_i64().unwrap() < active_part["g"].as_i64().unwrap());
    assert_eq!(none_part["r"].as_i64().unwrap(), 0);
    assert_eq!(none_part["g"].as_i64().unwrap(), 48);
    assert_eq!(none_part["b"].as_i64().unwrap(), 23);
    assert!(configured_part["g"].as_i64().unwrap() > 0);
    assert!(configured_part["g"].as_i64().unwrap() < active_part["g"].as_i64().unwrap());
    assert_eq!(dance_page["r"].as_i64().unwrap(), 0);
    assert_eq!(dance_page["g"].as_i64().unwrap(), 60);
    assert_eq!(dance_page["b"].as_i64().unwrap(), 60);
}

#[test]
fn fn_overlay_dims_fx_grid_cells_when_dance_mode_is_fx() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.ui.fn_held = true;
    runner.active_dance_mode = "fx".into();
    runner.dance_mode = "fx".into();

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);
    let mid_cell = cells[display_index(2, 2)].as_object().unwrap();
    let fx_page = cells[display_index(GRID_WIDTH - 1, 2)].as_object().unwrap();
    let part_cell = cells[display_index(0, 0)].as_object().unwrap();

    assert!(mid_cell["r"].as_i64().unwrap() < 20);
    assert!(mid_cell["g"].as_i64().unwrap() < 20);
    assert!(mid_cell["b"].as_i64().unwrap() < 20);
    assert!(fx_page["g"].as_i64().unwrap() > 100 && fx_page["g"].as_i64().unwrap() < 200);
    assert!(fx_page["g"].as_i64().unwrap() > 0 || fx_page["b"].as_i64().unwrap() > 0);
    assert!(part_cell["g"].as_i64().unwrap() > 0);
}
