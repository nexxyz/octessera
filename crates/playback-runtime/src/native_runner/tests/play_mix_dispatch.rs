use super::*;

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn press(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    let mut messages = input(runner, json!({ "type": "grid_press", "x": x, "y": y }));
    messages.extend(input(
        runner,
        json!({ "type": "grid_release", "x": x, "y": y }),
    ));
    messages
}

fn open_page(runner: &mut NativeRunner, row: usize, mode: &str) {
    let _ = input(runner, json!({ "type": "button_fn", "pressed": true }));
    let _ = press(runner, GRID_WIDTH - 1, row);
    let _ = input(runner, json!({ "type": "button_fn", "pressed": false }));
    assert_eq!(runner.active_play_mode, mode);
}

#[test]
fn mix_and_pan_presses_edit_their_menu_rows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    open_page(&mut runner, 0, "mix");
    let _ = press(&mut runner, 2, 3);
    assert_eq!(runner.instruments[2].volume, 43);
    assert_eq!(
        runner.menu.number_for_key("instruments.2.mixer.volume"),
        Some(43)
    );

    open_page(&mut runner, 1, "pan");
    let _ = press(&mut runner, 0, 1);
    assert_eq!(runner.instruments[1].pan_pos, 0);
    assert_eq!(
        runner.menu.number_for_key("instruments.1.mixer.panPos"),
        Some(0)
    );
}

#[test]
fn pan_on_a_bus_routed_instrument_edits_the_bus_row_and_keeps_its_own_pan() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner
        .menu
        .set_enum_value_for_key("instruments.3.mixer.route", "fx_bus_2"));
    runner
        .apply_or_schedule_menu_key("instruments.3.mixer.route")
        .unwrap();
    open_page(&mut runner, 1, "pan");
    let _ = press(&mut runner, 7, 3);
    assert_eq!(runner.fx_buses[1].pan_pos, 32);
    assert_eq!(runner.menu.number_for_key("mixer.buses.1.panPos"), Some(32));
    assert_eq!(runner.instruments[3].pan_pos, 32);
}

#[test]
fn mix_press_under_xy_modulation_moves_the_base_it_returns_to() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 30;
    runner.xy.touch = NativeXyTouch {
        x: 0.5,
        y: 0.5,
        display_x: 0.5,
        display_y: 0.5,
        active: true,
    };
    runner.xy.x_binding = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.refresh_xy_runtime_sources();
    runner.process_modulation_step(false).unwrap();
    open_page(&mut runner, 0, "mix");

    let _ = press(&mut runner, 0, 5);
    runner.set_param_binding_target("xy:x", None);

    assert_eq!(runner.instruments[0].volume, 71);
    assert_eq!(
        runner.menu.number_for_key("instruments.0.mixer.volume"),
        Some(71)
    );
}
