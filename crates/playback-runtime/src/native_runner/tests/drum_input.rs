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

fn press(runner: &mut NativeRunner, x: usize, y: usize) {
    input(runner, json!({ "type": "grid_press", "x": x, "y": y }));
}

fn with_modifier(runner: &mut NativeRunner, modifier: &str, x: usize, y: usize) {
    input(runner, json!({ "type": modifier, "pressed": true }));
    press(runner, x, y);
    input(runner, json!({ "type": modifier, "pressed": false }));
}

fn assign_voice(runner: &mut NativeRunner, voice: usize) {
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            format!("drum.assign:0:{voice}"),
        ))
        .unwrap();
}

fn set_stamp(runner: &mut NativeRunner, tune: i32, fine: i32, level: &str) {
    for (key, value) in [("tuneSemis", tune), ("tuneCents", fine)] {
        let key = format!("instruments.0.cell.{key}");
        runner.menu.set_number_value_for_key(&key, value);
        runner.apply_or_schedule_menu_key(&key).unwrap();
    }
    runner
        .menu
        .set_enum_value_for_key("instruments.0.cell.level", level);
    runner
        .apply_or_schedule_menu_key("instruments.0.cell.level")
        .unwrap();
}

#[test]
fn drum_rows_and_columns_stamp_every_cell_and_a_fully_stamped_line_clears() {
    let mut runner = drum_runner();
    assign_voice(&mut runner, 1);
    press(&mut runner, 3, 0);
    assert_eq!(cell(&runner, 3, 0).unwrap()["voice"], 1);

    with_modifier(&mut runner, "button_shift", 3, 0);
    for x in 0..GRID_WIDTH {
        assert_eq!(cell(&runner, x, 0).unwrap()["voice"], 1);
    }
    with_modifier(&mut runner, "button_shift", 5, 0);
    for x in 0..GRID_WIDTH {
        assert!(cell(&runner, x, 0).is_none());
    }

    assign_voice(&mut runner, 2);
    press(&mut runner, 4, 3);
    with_modifier(&mut runner, "button_combined_modifier", 4, 1);
    for y in 0..GRID_HEIGHT {
        assert_eq!(cell(&runner, 4, y).unwrap()["voice"], 2);
    }
    assert!(cell(&runner, 3, 3).is_none());
}

#[test]
fn drum_cells_stamp_tune_fine_and_level_and_restamping_replaces_or_clears() {
    let mut runner = drum_runner();
    set_stamp(&mut runner, 7, -25, "High");
    assign_voice(&mut runner, 0);
    press(&mut runner, 1, 0);
    assert_eq!(
        cell(&runner, 1, 0).unwrap(),
        json!({ "x": 1, "y": 0, "voice": 0, "tuneSemis": 7, "tuneCents": -25, "level": "high" })
    );
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "V1 +7 -25c High"
    );

    set_stamp(&mut runner, -3, 0, "Base");
    press(&mut runner, 1, 0);
    assert_eq!(
        cell(&runner, 1, 0).unwrap(),
        json!({ "x": 1, "y": 0, "voice": 0, "tuneSemis": -3 })
    );
    press(&mut runner, 1, 0);
    assert!(cell(&runner, 1, 0).is_none());
    assert_eq!(runner.snapshot().unwrap()["display"]["toast"], "Cleared");
    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["cellStamp"],
        json!({ "tuneSemis": -3, "tuneCents": 0 })
    );
    let mut reloaded = drum_runner();
    reloaded.apply_config_payload(payload).unwrap();
    assert_eq!(
        reloaded.instruments[0].cell_stamp,
        NativeCellStamp {
            tune_semis: -3,
            tune_cents: 0,
            level: None,
        }
    );
    assert_eq!(
        reloaded.menu.number_for_key("instruments.0.cell.tuneSemis"),
        Some(-3)
    );
}

#[test]
fn stamped_drum_level_scales_velocity_and_follow_pitch_and_transpose_are_opt_in() {
    let mut runner = drum_runner();
    runner.instruments[0].drum_config["assignments"] = json!([
        { "x": 2, "y": 0, "voice": 2, "tuneSemis": 3, "tuneCents": 10, "level": "low" }
    ]);
    let route = |runner: &NativeRunner, transpose: i8, note: u8| {
        super::super::modulation_sampler::apply_sampler_assignments_for_instruments_routed(
            vec![MusicalEvent::NoteOn {
                channel: 0,
                note,
                velocity: 127,
                duration_ms: Some(100),
            }],
            &[platform_core::CellTriggerIntent {
                x: 2,
                y: 0,
                degree: 0,
                kind: platform_core::CellTriggerKind::Activate,
            }],
            0,
            &runner.instruments,
            None,
            transpose,
            None,
        )
        .cells
    };
    let hit = route(&runner, 5, 64)[0].clone();
    assert_eq!((hit.pitch_cents, hit.velocity), (310, 45));
    runner.instruments[0].drum_config["followPitch"] = json!(true);
    assert_eq!(route(&runner, 5, 64)[0].pitch_cents, 710);
    runner.instruments[0].drum_config["followTranspose"] = json!(true);
    assert_eq!(route(&runner, 5, 64)[0].pitch_cents, 1210);
}

#[test]
fn follow_transpose_lets_a_layer_that_only_feeds_drums_or_samples_transpose() {
    let mut runner = drum_runner();
    for slot in 0..4 {
        runner.instruments[slot].kind = "drum".into();
    }
    runner.play_transpose_selected[0] = true;
    runner.play_transpose_enabled[0] = true;
    runner.play_transpose_offsets[0] = 5;
    assert_eq!(runner.play_transpose_offsets_for_routing()[0], 0);

    runner.instruments[0].drum_config["followTranspose"] = json!(true);
    assert_eq!(runner.play_transpose_offsets_for_routing()[0], 5);

    runner.instruments[0].drum_config["followTranspose"] = json!(false);
    runner.instruments[0].kind = "sampler".into();
    assert_eq!(runner.play_transpose_offsets_for_routing()[0], 0);
    runner.instruments[0].sample_follow_transpose = true;
    assert_eq!(runner.play_transpose_offsets_for_routing()[0], 5);
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
    assign_voice(&mut runner, 0);
    press(&mut runner, 2, 0);
    runner.drum_assign = None;
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
    assign_voice(&mut runner, 1);
    press(&mut runner, 2, 0);
    assign_voice(&mut runner, 2);
    press(&mut runner, 2, 7);
    runner.drum_assign = None;
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
            RunnerMessage::CellHits { hits } => hits.clone(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        hits,
        vec![crate::protocol::CellHit {
            instrument_slot: 0,
            voice: 3,
            cell: 2,
            pitch_cents: -1200,
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
        RunnerMessage::CellHits { .. }
            | RunnerMessage::MusicalEvents { .. }
            | RunnerMessage::MidiEvents { .. }
    )));
}
