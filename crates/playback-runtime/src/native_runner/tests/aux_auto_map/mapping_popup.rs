use super::*;

fn device_input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn fn_main_click(runner: &mut NativeRunner) {
    runner.skip_startup_splash();
    device_input(runner, json!({ "type": "button_fn", "pressed": true }));
    device_input(runner, json!({ "type": "encoder_press", "id": "main" }));
    device_input(runner, json!({ "type": "button_fn", "pressed": false }));
}

fn assert_mapping_entry(lines: &[String], prefix: &str, source: &str, label: &str) {
    let rows = lines
        .iter()
        .filter(|line| line.starts_with(prefix))
        .collect::<Vec<_>>();
    assert!(rows.iter().any(|line| line.contains(source)), "{prefix}");
    for word in label.split_whitespace() {
        assert!(
            rows.iter().any(|line| line.contains(word)),
            "{prefix} {word}"
        );
    }
}

#[test]
fn fn_main_opens_scrollable_aux_mapping_popup_and_aux_inputs_are_ignored() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = synth_stack(&runner, "Filter");
    runner.menu.state.cursor = 1;
    runner.menu.state.editing = true;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });
    runner.shift_aux_bindings[2] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: Some(NativeMenuAction::PlatformEffect("store.refresh".into())),
    });

    fn_main_click(&mut runner);
    let snapshot = runner.snapshot().unwrap();
    let lines = &runner.display.help_popup.as_ref().unwrap().lines;
    assert_eq!(snapshot["display"]["title"], "AUX MAP");
    assert_eq!(lines[0], "Synth Filter");
    assert_mapping_entry(lines, "A1 T ", "custom:", "Master Vol");
    assert_mapping_entry(lines, "A1 C ", "custom:", "MIDI Panic");
    assert_mapping_entry(lines, "A2 T ", "auto:", "Res");
    assert_mapping_entry(lines, "A3 T ", "auto:", "Env");
    assert_mapping_entry(lines, "S1 T ", "-", "-");
    assert_mapping_entry(lines, "S3 T ", "custom:", "Master Vol");
    assert_mapping_entry(lines, "S3 C ", "custom:", "Refresh");
    assert!(lines.iter().all(|line| line.chars().count() <= 19));
    assert!(!runner.display.ui.fn_held);
    assert!(runner.menu.state.editing);

    let volume = runner.display.ui.master_volume;
    device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    assert_eq!(runner.display.help_popup.as_ref().unwrap().scroll, 1);
    let turn_output = device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
    );
    let click_output = device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "aux1" }),
    );
    assert_eq!(runner.display.ui.master_volume, volume);
    assert!(!turn_output
        .iter()
        .chain(&click_output)
        .any(|message| matches!(
            message,
            RunnerMessage::PlatformEffects { effects }
                if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic))
        )));
    assert!(runner.menu.state.editing);

    device_input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    assert!(!runner.display.aux_mapping_peek_visible);
    assert!(runner.display.help_popup.is_none());
    assert!(runner.menu.state.editing);
}

#[test]
fn aux_mapping_popup_shows_empty_slots_stopped_and_playing_and_help_has_priority() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.aux_auto_map_enabled = false;
    fn_main_click(&mut runner);

    let popup = runner.display.help_popup.as_ref().unwrap();
    assert_eq!(popup.title, "AUX MAP");
    assert!(popup.lines[1..]
        .iter()
        .all(|line| line.chars().count() <= 19 && line.ends_with("-")));
    for bank in ["A", "S"] {
        for slot in 1..=3 {
            for kind in ["T", "C"] {
                let prefix = format!("{bank}{slot} {kind} ");
                assert!(popup
                    .lines
                    .iter()
                    .any(|line| line.starts_with(&prefix) && line.ends_with("-")));
            }
        }
    }
    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);

    fn_main_click(&mut runner);
    assert!(runner.display.help_popup.is_none() && !runner.display.aux_mapping_peek_visible);

    runner.transport.transport = RuntimeTransportState::Playing;
    fn_main_click(&mut runner);
    assert!(runner.display.aux_mapping_peek_visible);
    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(!runner.display.aux_mapping_peek_visible);
    assert!(runner.display.help_popup.is_none());

    fn_main_click(&mut runner);
    device_input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": true }),
    );
    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(!runner.display.aux_mapping_peek_visible);
    assert!(runner.display.help_popup.is_none());
    device_input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": false }),
    );

    assert!(runner.menu.focus_item_key("masterVolume"));
    fn_main_click(&mut runner);
    device_input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    device_input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": true }),
    );
    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(!runner.display.aux_mapping_peek_visible);
    assert!(runner
        .display
        .help_popup
        .as_ref()
        .is_some_and(|popup| popup.title.starts_with("Help:")));
    let menu_path = runner.menu.current_focus_path();
    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(runner.display.help_popup.is_none());
    assert_eq!(runner.menu.current_focus_path(), menu_path);
    device_input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    device_input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": false }),
    );
}

#[test]
fn prolonged_fn_hold_does_not_open_aux_mapping_popup() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    device_input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    runner.test_set_display_time(Instant::now() + Duration::from_secs(2));

    assert!(!runner.display.aux_mapping_peek_visible);
    assert!(runner.display.help_popup.is_none());
}
