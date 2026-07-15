use super::*;

#[test]
pub(crate) fn life_palette_surfaces_in_runtime_snapshot_led_rgb() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    select_behavior(&mut runner, "life");
    runner.oled_splash_text.clear();
    runner.oled_splash_until = None;

    let stable_messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 3, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    let stable_snapshot = snapshot_from(&stable_messages);
    let stable_leds = led_cells(&stable_snapshot);

    assert_eq!(
        stable_leds[display_index(0, 0)],
        led_rgb(platform_core::palette::BEHAVIOR_DIM_GREEN)
    );
    assert_eq!(
        stable_leds[display_index(3, 3)],
        led_rgb(platform_core::palette::BEHAVIOR_PRIMARY_YELLOW)
    );

    let active_messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "behavior_action", "actionType": "spawnGlider" }),
            request_snapshot: None,
        })
        .unwrap();
    let active_snapshot = snapshot_from(&active_messages);
    let active_leds = led_cells(&active_snapshot);

    assert_eq!(
        active_leds[display_index(1, 0)],
        led_rgb(platform_core::palette::BEHAVIOR_PRIMARY_MAGENTA)
    );
    assert_eq!(
        active_leds[display_index(7, 7)],
        led_rgb(platform_core::palette::BEHAVIOR_DIM_GREEN)
    );
}

#[test]
pub(crate) fn water_themed_backgrounds_surface_without_hiding_live_cells() {
    for behavior_id in ["bubbles", "raindrops"] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        select_behavior(&mut runner, behavior_id);
        runner.oled_splash_text.clear();
        runner.oled_splash_until = None;

        let snapshot = runner.snapshot().unwrap();
        let leds = led_cells(&snapshot);
        assert_eq!(
            leds[display_index(0, 0)],
            led_rgb(platform_core::palette::BLUE),
            "{behavior_id} inactive background"
        );

        let action = if behavior_id == "bubbles" {
            "addBubble"
        } else {
            "dropNow"
        };
        runner.trigger_behavior_action(action.into()).unwrap();
        if behavior_id == "raindrops" {
            runner.engine.tick(runner.bpm as f32).unwrap();
        }
        let active_snapshot = runner.snapshot().unwrap();
        let active_leds = led_cells(&active_snapshot);
        let model = runner.engine.model().unwrap();
        let live_index = model
            .cells
            .iter()
            .position(|cell| *cell)
            .unwrap_or_else(|| panic!("{behavior_id} should render a live cell after action"));
        let x = live_index % GRID_WIDTH;
        let y = live_index / GRID_WIDTH;

        assert_ne!(
            active_leds[display_index(x, y)],
            led_rgb(platform_core::palette::BLUE),
            "{behavior_id} live cell must contrast with water background"
        );
    }
}

#[test]
pub(crate) fn ant_palette_keeps_ant_and_trail_visible_in_runtime_snapshot_led_rgb() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    select_behavior(&mut runner, "ant");
    runner.oled_splash_text.clear();
    runner.oled_splash_until = None;

    runner
        .active_engine_input_result(platform_core::DeviceInput::GridPress { x: 2, y: 3 })
        .unwrap();
    let spawn_snapshot = runner.snapshot().unwrap();
    let spawn_leds = led_cells(&spawn_snapshot);

    assert_eq!(
        spawn_leds[display_index(0, 0)],
        led_rgb(platform_core::palette::BLACK)
    );
    assert_eq!(
        spawn_leds[display_index(2, 3)],
        led_rgb(platform_core::palette::YELLOW)
    );

    runner.engine.tick(runner.bpm as f32).unwrap();
    let tick_snapshot = runner.snapshot().unwrap();
    let tick_leds = led_cells(&tick_snapshot);

    assert_eq!(
        tick_leds[display_index(2, 3)],
        led_rgb(platform_core::palette::YELLOW)
    );
    assert_eq!(
        tick_leds[display_index(3, 3)],
        led_rgb(platform_core::palette::YELLOW)
    );
    assert_ne!(
        tick_leds[display_index(2, 3)],
        led_rgb(platform_core::palette::BLACK)
    );
    assert_ne!(
        tick_leds[display_index(3, 3)],
        led_rgb(platform_core::palette::BLACK)
    );
}
