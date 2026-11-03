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

fn tile(runner: &NativeRunner, x: usize, y: usize) -> Option<Value> {
    runner.instruments[0].drum_config["assignments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tile| tile["x"] == x && tile["y"] == y)
        .cloned()
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
        let key = format!("instruments.0.tile.{key}");
        runner.menu.set_number_value_for_key(&key, value);
        runner.apply_or_schedule_menu_key(&key).unwrap();
    }
    runner
        .menu
        .set_enum_value_for_key("instruments.0.tile.level", level);
    runner
        .apply_or_schedule_menu_key("instruments.0.tile.level")
        .unwrap();
}

#[test]
fn drum_rows_and_columns_stamp_every_tile_and_a_fully_stamped_line_clears() {
    let mut runner = drum_runner();
    assign_voice(&mut runner, 1);
    press(&mut runner, 3, 0);
    assert_eq!(tile(&runner, 3, 0).unwrap()["voice"], 1);

    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "V2 +0 +0c Base"
    );

    with_modifier(&mut runner, "button_shift", 3, 0);
    for x in 0..GRID_WIDTH {
        assert_eq!(tile(&runner, x, 0).unwrap()["voice"], 1);
    }
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "row +0 +0c Base"
    );
    with_modifier(&mut runner, "button_shift", 5, 0);
    for x in 0..GRID_WIDTH {
        assert!(tile(&runner, x, 0).is_none());
    }
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "Cleared row"
    );

    assign_voice(&mut runner, 2);
    press(&mut runner, 4, 3);
    with_modifier(&mut runner, "button_combined_modifier", 4, 1);
    for y in 0..GRID_HEIGHT {
        assert_eq!(tile(&runner, 4, y).unwrap()["voice"], 2);
    }
    assert!(tile(&runner, 3, 3).is_none());
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "col +0 +0c Base"
    );
    with_modifier(&mut runner, "button_combined_modifier", 4, 6);
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "Cleared col"
    );
}

#[test]
fn a_press_outside_the_grid_stamps_nothing_shows_no_toast_and_leaves_the_config_clean() {
    let mut runner = drum_runner();
    assign_voice(&mut runner, 0);
    runner.display.toast = None;
    runner.config_dirty = false;
    let before = runner.instruments[0].drum_config["assignments"].clone();
    for (shift, combined) in [(false, false), (true, false), (false, true)] {
        runner.display.ui.shift_held = shift;
        runner.display.ui.combined_modifier_held = combined;
        runner.handle_drum_assignment_grid_press(GRID_WIDTH, GRID_HEIGHT);
    }
    assert_eq!(runner.instruments[0].drum_config["assignments"], before);
    assert!(runner.display.toast.is_none());
    assert!(!runner.config_dirty);

    runner.instruments[1].kind = "sampler".into();
    runner.sample_assign = Some((1, 0));
    runner.drum_assign = None;
    runner.display.ui.shift_held = true;
    runner.handle_sample_assignment_grid_press(0, GRID_HEIGHT);
    assert!(runner.instruments[1].sample_assignments.is_empty());
    assert!(runner.display.toast.is_none());
    assert!(!runner.config_dirty);
}

#[test]
fn drum_tiles_stamp_tune_fine_and_level_and_restamping_replaces_or_clears() {
    let mut runner = drum_runner();
    set_stamp(&mut runner, 7, -25, "High");
    assign_voice(&mut runner, 0);
    press(&mut runner, 1, 0);
    assert_eq!(
        tile(&runner, 1, 0).unwrap(),
        json!({ "x": 1, "y": 0, "voice": 0, "tuneSemis": 7, "tuneCents": -25, "level": "high" })
    );
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "V1 +7 -25c High"
    );

    set_stamp(&mut runner, -3, 0, "Base");
    press(&mut runner, 1, 0);
    assert_eq!(
        tile(&runner, 1, 0).unwrap(),
        json!({ "x": 1, "y": 0, "voice": 0, "tuneSemis": -3 })
    );
    press(&mut runner, 1, 0);
    assert!(tile(&runner, 1, 0).is_none());
    assert_eq!(runner.snapshot().unwrap()["display"]["toast"], "Cleared");
    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["tileStamp"],
        json!({ "tuneSemis": -3, "tuneCents": 0 })
    );
    let mut reloaded = drum_runner();
    reloaded.apply_config_payload(payload).unwrap();
    assert_eq!(
        reloaded.instruments[0].tile_stamp,
        NativeTileStamp {
            tune_semis: -3,
            tune_cents: 0,
            level: None,
        }
    );
    assert_eq!(
        reloaded.menu.number_for_key("instruments.0.tile.tuneSemis"),
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
            &[platform_core::TileTriggerIntent {
                x: 2,
                y: 0,
                degree: 0,
                kind: platform_core::TileTriggerKind::Activate,
            }],
            0,
            &runner.instruments,
            None,
            transpose,
            None,
        )
        .tiles
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
fn drum_assignment_leds_use_world_to_display_index_and_override_play_fn_navigation() {
    let mut runner = drum_runner();
    assign_voice(&mut runner, 1);
    press(&mut runner, 2, 0);
    assign_voice(&mut runner, 2);
    press(&mut runner, 2, 7);
    runner.drum_assign = None;
    runner.active_play_mode = "xy".into();
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::PlatformEffect(
            "drum.assign:0:1".into(),
        ))
        .unwrap();
    input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    let leds = led_tiles(&runner.snapshot().unwrap());
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
