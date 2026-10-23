use super::*;

fn send(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: Some(true),
        })
        .unwrap()
}

fn press_main(runner: &mut NativeRunner) -> Value {
    snapshot_from(&send(
        runner,
        json!({ "type": "encoder_press", "id": "main" }),
    ))
}

fn tone_events(messages: &[RunnerMessage]) -> Vec<platform_core::MusicalEvent> {
    routed_events(messages, false)
}

fn start_test(runner: &mut NativeRunner) -> Value {
    send(runner, json!({ "type": "other" }));
    assert!(runner.menu.focus_item_key("system.hardwareTest"));
    let snapshot = press_main(runner);
    assert_eq!(snapshot["display"]["title"], "Confirm Hardware Test");
    send(
        runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    press_main(runner)
}

#[test]
pub(crate) fn hardware_test_walks_leds_pads_keys_encoders_and_sound_from_the_device() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let snapshot = start_test(&mut runner);
    assert_eq!(snapshot["display"]["title"], "Test 1/5: Grid LEDs");
    let leds = led_cells(&snapshot);
    assert_eq!(
        leds[display_index(0, 0)],
        led_rgb(platform_core::palette::MAGENTA)
    );
    assert_eq!(
        leds[display_index(7, 7)],
        led_rgb(platform_core::palette::WHITE)
    );

    assert_eq!(
        press_main(&mut runner)["display"]["title"],
        "Test 2/5: Pads"
    );
    send(&mut runner, json!({ "type": "grid_press", "x": 3, "y": 4 }));
    let snapshot = snapshot_from(&send(
        &mut runner,
        json!({ "type": "grid_release", "x": 3, "y": 4 }),
    ));
    assert_eq!(snapshot["display"]["lines"][1], "Seen 1/64");
    assert_eq!(
        led_cells(&snapshot)[display_index(3, 4)],
        led_rgb(platform_core::palette::GREEN)
    );
    assert!(!runner.engine.model().unwrap().cells[4 * GRID_WIDTH + 3]);

    assert_eq!(
        press_main(&mut runner)["display"]["title"],
        "Test 3/5: Keys"
    );
    let snapshot = snapshot_from(&send(
        &mut runner,
        json!({ "type": "button_a", "pressed": true }),
    ));
    assert_eq!(
        snapshot["display"]["title"], "Test 3/5: Keys",
        "Back is under test here"
    );
    assert_eq!(snapshot["display"]["lines"][1], "Back  ok");
    assert_eq!(
        snapshot["neoKeyLeds"]["back"],
        json!(platform_core::palette::GREEN)
    );

    assert_eq!(
        press_main(&mut runner)["display"]["title"],
        "Test 4/5: Encoders"
    );
    send(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux2", "delta": 1 }),
    );
    let snapshot = snapshot_from(&send(
        &mut runner,
        json!({ "type": "encoder_press", "id": "aux2" }),
    ));
    assert_eq!(snapshot["display"]["lines"][3], "Aux2 turn ok click ok");

    let messages = send(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert_eq!(
        snapshot_from(&messages)["display"]["title"],
        "Test 5/5: Sound"
    );
    let tone = platform_core::MusicalEvent::NoteOn {
        channel: 0,
        note: 69,
        velocity: 100,
        duration_ms: Some(800),
    };
    assert_eq!(tone_events(&messages), vec![tone.clone()]);
    let again = send(&mut runner, json!({ "type": "button_s", "pressed": true }));
    assert_eq!(tone_events(&again), vec![tone]);
    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);

    let summary = press_main(&mut runner);
    assert_eq!(summary["display"]["title"], "Hardware Test");
    assert_eq!(summary["display"]["lines"][0], "Pads 1/64");
    assert_eq!(summary["display"]["lines"][1], "Keys 1/4");
    assert_eq!(summary["display"]["lines"][2], "Encoders 2/7");

    press_main(&mut runner);
    assert!(runner.display.hardware_test.is_none());
    assert!(runner.is_canonical_menu_presentation());
}

#[test]
pub(crate) fn back_leaves_the_hardware_test_outside_the_key_step() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    start_test(&mut runner);
    press_main(&mut runner);
    send(&mut runner, json!({ "type": "button_a", "pressed": true }));
    assert!(runner.display.hardware_test.is_none());
}
