use super::*;

fn board_runner() -> NativeRunner {
    NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap()
}

fn press_main(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

fn emitted(messages: &[RunnerMessage], expected: RuntimePlatformEffect) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            RunnerMessage::PlatformEffects { effects } if effects == &vec![expected.clone()]
        )
    })
}

#[test]
fn turning_wifi_off_asks_first_and_turning_it_on_does_not() {
    let mut runner = board_runner();

    assert!(runner.menu.focus_item_key("wifi.off"));
    let asked = press_main(&mut runner);
    assert!(!emitted(
        &asked,
        RuntimePlatformEffect::WifiRadio { enabled: false }
    ));
    let dialog = runner.display.confirm_dialog.as_ref().unwrap();
    assert_eq!(dialog.title, "Turn Wi-Fi Off");
    assert!(dialog.lines.iter().all(|line| line.chars().count() <= 20));

    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;
    let off = press_main(&mut runner);
    assert!(emitted(
        &off,
        RuntimePlatformEffect::WifiRadio { enabled: false }
    ));
    assert_eq!(runner.display.toast.as_ref().unwrap().message, "Wi-Fi off");

    assert!(runner.menu.focus_item_key("wifi.on"));
    let on = press_main(&mut runner);
    assert!(emitted(
        &on,
        RuntimePlatformEffect::WifiRadio { enabled: true }
    ));
    assert!(runner.display.confirm_dialog.is_none());
    assert_eq!(runner.display.toast.as_ref().unwrap().message, "Wi-Fi on");
}

#[test]
fn cancelling_wifi_off_changes_nothing() {
    let mut runner = board_runner();

    assert!(runner.menu.focus_item_key("wifi.off"));
    let _ = press_main(&mut runner);
    let cancelled = press_main(&mut runner);
    assert!(!emitted(
        &cancelled,
        RuntimePlatformEffect::WifiRadio { enabled: false }
    ));
    assert_eq!(runner.display.toast.as_ref().unwrap().message, "Cancelled");
}

#[test]
fn wifi_switch_is_board_only() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(!runner.menu.focus_item_key("wifi.off"));
    assert!(!runner.menu.focus_item_key("wifi.on"));
}
