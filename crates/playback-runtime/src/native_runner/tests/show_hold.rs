use super::*;

fn press_action(runner: &mut NativeRunner, key: &str) -> Vec<RunnerMessage> {
    assert!(runner.menu.focus_item_key(key));
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
fn starting_live_show_mode_asks_first_and_end_does_not() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let asked = press_action(&mut runner, "maintenance.hold");
    assert!(!emitted(
        &asked,
        RuntimePlatformEffect::MaintenanceHold { active: true }
    ));
    let dialog = runner.display.confirm_dialog.as_ref().unwrap();
    assert_eq!(dialog.title, "Live Show Mode");
    assert!(dialog.lines.iter().any(|line| line.contains("Wi-Fi off")));
    assert!(dialog.lines.iter().all(|line| line.chars().count() <= 20));

    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;
    let held = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(emitted(
        &held,
        RuntimePlatformEffect::MaintenanceHold { active: true }
    ));
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Live Show Mode: 48h"
    );

    let released = press_action(&mut runner, "maintenance.release");
    assert!(emitted(
        &released,
        RuntimePlatformEffect::MaintenanceHold { active: false }
    ));
    assert!(runner.display.confirm_dialog.is_none());
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Live Show Mode off"
    );
}

#[test]
fn cancelling_live_show_mode_changes_nothing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let _ = press_action(&mut runner, "maintenance.hold");
    let cancelled = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!emitted(
        &cancelled,
        RuntimePlatformEffect::MaintenanceHold { active: true }
    ));
    assert_eq!(runner.display.toast.as_ref().unwrap().message, "Cancelled");
}

#[test]
fn show_hold_is_board_only() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(!runner.menu.focus_item_key("maintenance.hold"));
}
