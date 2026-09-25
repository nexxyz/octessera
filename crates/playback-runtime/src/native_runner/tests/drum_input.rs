use super::*;

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn drum_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    runner.apply_config_payload(payload).unwrap();
    runner
}

fn cell(runner: &NativeRunner, x: usize, y: usize) -> Option<Value> {
    runner.instruments[0].drum_config["assignments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cell| cell["x"] == x && cell["y"] == y)
        .cloned()
}

fn oled_lines(snapshot: &Value) -> Vec<&str> {
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines.len() <= OLED_BODY_ROWS);
    lines
        .iter()
        .map(|line| {
            let line = line.as_str().unwrap();
            assert!(line.chars().count() <= 28);
            line
        })
        .collect()
}

#[test]
fn drum_assign_shift_bottom_row_and_combined_column_toggle_each_cell_independently() {
    let mut runner = drum_runner();
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "drum.assign:0:1".into(),
        ))
        .unwrap();
    input(&mut runner, json!({ "type": "grid_press", "x": 3, "y": 0 }));
    assert_eq!(cell(&runner, 3, 0).unwrap()["voice"], 1);
    input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": true }),
    );
    input(&mut runner, json!({ "type": "grid_press", "x": 3, "y": 0 }));
    input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": false }),
    );
    assert!(cell(&runner, 3, 0).is_none());
    for x in 0..GRID_WIDTH {
        if x != 3 {
            assert_eq!(cell(&runner, x, 0).unwrap()["voice"], 1);
        }
    }
    assert!(cell(&runner, 3, 7).is_none());

    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "drum.assign:0:2".into(),
        ))
        .unwrap();
    input(&mut runner, json!({ "type": "grid_press", "x": 4, "y": 3 }));
    input(
        &mut runner,
        json!({ "type": "button_combined_modifier", "pressed": true }),
    );
    input(&mut runner, json!({ "type": "grid_press", "x": 4, "y": 1 }));
    input(
        &mut runner,
        json!({ "type": "button_combined_modifier", "pressed": false }),
    );
    for y in 0..GRID_HEIGHT {
        if y == 3 {
            assert!(cell(&runner, 4, y).is_none());
        } else {
            assert_eq!(cell(&runner, 4, y).unwrap()["voice"], 2);
        }
    }
    assert_eq!(cell(&runner, 0, 0).unwrap()["voice"], 1);
    assert_eq!(cell(&runner, 4, 0).unwrap()["voice"], 2);
}

#[test]
fn drum_replacement_resets_only_local_tune_and_back_steps_through_cell_tune() {
    let mut runner = drum_runner();
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "drum.assign:0:0".into(),
        ))
        .unwrap();
    for x in [1, 2] {
        input(&mut runner, json!({ "type": "grid_press", "x": x, "y": 0 }));
    }
    input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    assert!(runner.drum_assign.is_none());
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "drum.cellTune:0".into(),
        ))
        .unwrap();
    let choosing = runner.snapshot().unwrap();
    assert_eq!(choosing["display"]["title"], "Cell Tune");
    assert_eq!(oled_lines(&choosing), ["Choose drum cell"]);
    assert!(choosing["selectedRow"].is_null());
    input(&mut runner, json!({ "type": "grid_press", "x": 7, "y": 7 }));
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "No drum here"
    );
    assert_eq!(runner.drum_cell_tune, Some((0, None)));
    input(&mut runner, json!({ "type": "grid_press", "x": 1, "y": 0 }));
    let selected = runner.snapshot().unwrap();
    assert_eq!(oled_lines(&selected), ["Kick (1,0)", "Tune +0 st"]);
    input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 5 }),
    );
    let turned = runner.snapshot().unwrap();
    assert_eq!(oled_lines(&turned), ["Kick (1,0)", "Tune +5 st"]);
    assert_eq!(cell(&runner, 1, 0).unwrap()["tuneSemis"], 5);
    input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": -7 }),
    );
    assert_eq!(
        oled_lines(&runner.snapshot().unwrap()),
        ["Kick (1,0)", "Tune -2 st"]
    );
    assert!(cell(&runner, 2, 0).unwrap().get("tuneSemis").is_none());
    input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    assert_eq!(runner.drum_cell_tune, Some((0, None)));
    assert_eq!(
        oled_lines(&runner.snapshot().unwrap()),
        ["Choose drum cell"]
    );
    input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    assert!(runner.drum_cell_tune.is_none());
    assert_ne!(runner.snapshot().unwrap()["display"]["title"], "Cell Tune");
    assert!(runner.menu.focus_item_key("drum.cellTune.0"));
    let drum_menu = runner.snapshot().unwrap();
    assert_eq!(oled_lines(&drum_menu).len(), 7);
    assert_eq!(drum_menu["display"]["visibleRows"], 7);
    assert_eq!(drum_menu["display"]["totalRows"], 7);
    assert!(drum_menu["selectedRow"].as_u64().unwrap() < 7);

    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "drum.assign:0:2".into(),
        ))
        .unwrap();
    input(&mut runner, json!({ "type": "grid_press", "x": 1, "y": 0 }));
    assert_eq!(
        cell(&runner, 1, 0).unwrap(),
        json!({ "x": 1, "y": 0, "voice": 2 })
    );
    assert_eq!(cell(&runner, 2, 0).unwrap()["voice"], 0);
}

#[test]
fn cell_tune_oled_uses_current_kit_sound_not_fixed_voice_name() {
    let mut runner = drum_runner();
    runner.instruments[0].drum_config["voices"][0]["sound"] = json!("closed_hat");
    runner.assign_drum_cell(0, 0, 3, 0);
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "drum.cellTune:0".into(),
        ))
        .unwrap();
    input(&mut runner, json!({ "type": "grid_press", "x": 3, "y": 0 }));
    assert_eq!(
        oled_lines(&runner.snapshot().unwrap()),
        ["Closed Hat (3,0)", "Tune +0 st"]
    );
}

#[test]
fn play_drums_oled_keeps_slot_cursor_and_scroll_metadata_without_selectable_guidance() {
    let mut runner = drum_runner();
    assert!(runner.menu.focus_item_key("play.page.drums"));
    input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(runner.menu.focus_item_key("play.drums.slot"));
    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["display"]["title"], "/P/Drums");
    let lines = oled_lines(&snapshot);
    assert!(lines[0].contains("Slot"));
    assert_eq!(lines.last(), Some(&"Grid: tap to play"));
    assert_eq!(snapshot["selectedRow"], 0);
    assert_eq!(snapshot["display"]["scrollOffset"], 0);
    assert_eq!(snapshot["display"]["totalRows"], lines.len());
    assert_eq!(snapshot["display"]["visibleRows"], lines.len());
    assert_eq!(
        runner
            .menu
            .item_for_key("play.page.drums")
            .unwrap()
            .children
            .len(),
        1
    );
    input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let editing = runner.snapshot().unwrap();
    assert_eq!(editing["display"]["editing"], true);
    assert_eq!(oled_lines(&editing).last(), Some(&"Grid: tap to play"));
    input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    assert_eq!(
        oled_lines(&runner.snapshot().unwrap()).last(),
        Some(&"Grid: tap to play")
    );

    let mut empty = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(empty.menu.focus_item_key("play.page.drums"));
    assert!(oled_lines(&empty.snapshot().unwrap())
        .iter()
        .any(|line| line.contains("No Drum slot")));
    input(&mut empty, json!({ "type": "encoder_press", "id": "main" }));
    let no_slot = empty.snapshot().unwrap();
    assert_eq!(oled_lines(&no_slot), ["No Drum slot"]);
    assert!(!oled_lines(&no_slot)
        .iter()
        .any(|line| line.contains("Grid: tap")));
    assert!(no_slot["display"]["title"]
        .as_str()
        .unwrap()
        .contains("No Drum slot"));
    assert!(runner.menu.focus_item_key("play.page.xy"));
    assert!(!oled_lines(&runner.snapshot().unwrap())
        .iter()
        .any(|line| line.contains("Grid: tap")));
}

#[test]
fn play_drums_overlay_blacks_unassigned_behavior_cells_and_entire_grid_without_slot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    runner.apply_config_payload(payload).unwrap();
    runner.assign_drum_cell(0, 0, 2, 0);
    input(&mut runner, json!({ "type": "grid_press", "x": 3, "y": 0 }));
    let before = led_cells(&runner.snapshot().unwrap());
    assert_ne!(before[display_index(3, 0)], led_rgb([0, 0, 0]));
    input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    input(&mut runner, json!({ "type": "grid_press", "x": 7, "y": 6 }));
    input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    let leds = led_cells(&runner.snapshot().unwrap());
    assert_eq!(leds[display_index(3, 0)], led_rgb([0, 0, 0]));
    assert_eq!(
        leds[display_index(2, 0)],
        led_rgb(dim_rgb(platform_core::palette::WHITE, 3))
    );
    assert_eq!(
        leds.iter()
            .filter(|led| **led != led_rgb([0, 0, 0]))
            .count(),
        1
    );
    runner.play_drum_selected_slot = Some(1);
    assert!(led_cells(&runner.snapshot().unwrap())
        .iter()
        .all(|led| *led == led_rgb([0, 0, 0])));

    let mut empty = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    input(&mut empty, json!({ "type": "grid_press", "x": 3, "y": 0 }));
    assert_ne!(
        led_cells(&empty.snapshot().unwrap())[display_index(3, 0)],
        led_rgb([0, 0, 0])
    );
    input(&mut empty, json!({ "type": "button_fn", "pressed": true }));
    input(&mut empty, json!({ "type": "grid_press", "x": 7, "y": 6 }));
    input(&mut empty, json!({ "type": "button_fn", "pressed": false }));
    assert!(led_cells(&empty.snapshot().unwrap())
        .iter()
        .all(|led| *led == led_rgb([0, 0, 0])));
}

#[test]
fn drum_assignment_leds_use_world_to_display_index_and_override_play_fn_navigation() {
    let mut runner = drum_runner();
    runner.assign_drum_cell(0, 1, 2, 0);
    runner.assign_drum_cell(0, 2, 2, 7);
    runner.active_play_mode = "drums".into();
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "drum.assign:0:1".into(),
        ))
        .unwrap();
    input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    let leds = led_cells(&runner.snapshot().unwrap());
    assert_eq!(
        leds[display_index(2, 0)],
        led_rgb(platform_core::palette::WHITE)
    );
    assert_eq!(
        leds[display_index(2, 7)],
        led_rgb(dim_rgb(platform_core::palette::WHITE, 3))
    );
    assert_eq!(leds[display_index(7, 6)], led_rgb([0, 0, 0]));
    input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
}

#[test]
fn play_drums_fn_page_press_while_stopped_consumes_behavior_once_and_empty_is_silent() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    payload["runtimeConfig"]["instruments"][0]["drum"]["assignments"] =
        json!([{ "x": 2, "y": 0, "voice": 3, "tuneSemis": -12 }]);
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);
    input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    input(&mut runner, json!({ "type": "grid_press", "x": 7, "y": 6 }));
    input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    assert_eq!(runner.active_play_mode, "drums");
    runner.link_layers[0].trigger_probability_mode = "zero".into();
    runner.refresh_active_interpretation_profile();
    let press = input(&mut runner, json!({ "type": "grid_press", "x": 2, "y": 0 }));
    let hits = press
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::DrumHits { hits } => hits.clone(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        hits,
        vec![crate::protocol::DrumHit {
            instrument_slot: 0,
            voice: 3,
            tune_semis: -12,
            velocity: 100
        }]
    );
    assert!(!press.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { .. } | RunnerMessage::MidiEvents { .. }
    )));
    let release = input(
        &mut runner,
        json!({ "type": "grid_release", "x": 2, "y": 0 }),
    );
    let empty = input(&mut runner, json!({ "type": "grid_press", "x": 2, "y": 1 }));
    assert!(!release.iter().chain(empty.iter()).any(|message| matches!(
        message,
        RunnerMessage::DrumHits { .. }
            | RunnerMessage::MusicalEvents { .. }
            | RunnerMessage::MidiEvents { .. }
    )));
}
