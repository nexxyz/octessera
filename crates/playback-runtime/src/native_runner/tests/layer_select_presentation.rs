use super::*;

fn presents(runner: &mut NativeRunner, input: Value) -> bool {
    let generation = runner.display.transients.generation();
    runner
        .send_music_first(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
    runner.display.transients.generation() > generation
}

fn playing_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.display.toast = None;
    runner
}

#[test]
fn fn_layer_select_shows_its_toast_and_its_tile_release_presents_nothing() {
    let mut runner = playing_runner();

    let presented = [
        presents(&mut runner, json!({ "type": "button_fn", "pressed": true })),
        presents(&mut runner, json!({ "type": "grid_press", "x": 0, "y": 2 })),
        presents(
            &mut runner,
            json!({ "type": "grid_release", "x": 0, "y": 2 }),
        ),
        presents(
            &mut runner,
            json!({ "type": "button_fn", "pressed": false }),
        ),
    ];

    assert_eq!(presented, [true, true, false, true]);
    assert_eq!(runner.active_layer_index, 2);
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Layer: L3 life")
    );
}

#[test]
fn layer_select_tile_released_after_fn_still_presents_nothing() {
    let mut runner = playing_runner();

    presents(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    presents(&mut runner, json!({ "type": "grid_press", "x": 0, "y": 3 }));
    presents(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );

    assert!(!presents(
        &mut runner,
        json!({ "type": "grid_release", "x": 0, "y": 3 })
    ));
    assert_eq!(runner.active_layer_index, 3);
}

#[test]
fn other_tile_releases_still_present() {
    let mut runner = playing_runner();

    presents(&mut runner, json!({ "type": "grid_press", "x": 3, "y": 3 }));

    assert!(presents(
        &mut runner,
        json!({ "type": "grid_release", "x": 3, "y": 3 })
    ));
}
