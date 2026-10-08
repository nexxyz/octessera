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
fn show_hold_actions_emit_the_hold_effect_without_confirmation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let held = press_action(&mut runner, "maintenance.hold");
    assert!(emitted(
        &held,
        RuntimePlatformEffect::MaintenanceHold { active: true }
    ));
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Show hold: 48h"
    );

    let released = press_action(&mut runner, "maintenance.release");
    assert!(emitted(
        &released,
        RuntimePlatformEffect::MaintenanceHold { active: false }
    ));
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Show hold released"
    );
}

#[test]
fn show_hold_is_board_only() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(!runner.menu.focus_item_key("maintenance.hold"));
}
