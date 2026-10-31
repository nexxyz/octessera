use super::support::FakeHost;
use crate::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage,
    RuntimeConfig, RuntimePlatformEffect, RuntimeStoreResult,
};
use serde_json::json;
use std::time::{Duration, Instant};

#[test]
fn failed_manual_recording_stop_replays_typed_failure_to_clear_lifecycle() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let mut payload = runner.test_config_payload();
    payload["runtimeConfig"]["screenSleepSeconds"] = json!(1);
    payload["runtimeConfig"]["dimTimerSeconds"] = json!(0);
    runner.apply_config_payload(payload).unwrap();
    runner.test_set_display_time(Instant::now());
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RecordingStatus {
                ok: true,
                message: "Recording started".into(),
                active: true,
            },
        })
        .unwrap();
    let mut host = FakeHost {
        fail_recording_stop: true,
        ..FakeHost::default()
    };

    runtime
        .dispatch_runner_messages(
            vec![RunnerMessage::PlatformEffects {
                effects: vec![RuntimePlatformEffect::RecordingStop],
            }],
            &mut runner,
            &mut host,
        )
        .unwrap();

    let snapshot = runner
        .messages_with_snapshot()
        .unwrap()
        .into_iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot),
            _ => None,
        })
        .unwrap();
    assert!(
        snapshot["display"]["title"] == "RUNTIME ERROR"
            && snapshot["display"]["lines"]
                .as_array()
                .is_some_and(|lines| {
                    lines
                        .iter()
                        .any(|line| line.as_str().is_some_and(|line| line.contains("stop")))
                })
    );
    runner.test_advance_display_time(Duration::from_secs(1));
    runner.messages_with_snapshot().unwrap();
    runner.test_advance_display_time(Duration::from_secs(3));
    let snapshot = runner
        .messages_with_snapshot()
        .unwrap()
        .into_iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot),
            _ => None,
        })
        .unwrap();
    assert_eq!(snapshot["display"]["off"], true);
}

#[test]
fn failed_reboot_recording_finalization_clears_lifecycle_and_keeps_transition_failure() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let mut payload = runner.test_config_payload();
    payload["runtimeConfig"]["screenSleepSeconds"] = json!(1);
    payload["runtimeConfig"]["dimTimerSeconds"] = json!(0);
    runner.apply_config_payload(payload).unwrap();
    runner.test_set_display_time(Instant::now());
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RecordingStatus {
                ok: true,
                message: "Recording started".into(),
                active: true,
            },
        })
        .unwrap();
    let mut host = FakeHost {
        fail_recording_transition: true,
        ..FakeHost::default()
    };

    runtime
        .dispatch_runner_messages(
            vec![RunnerMessage::PlatformEffects {
                effects: vec![RuntimePlatformEffect::Reboot],
            }],
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(runtime.latched_errors().iter().any(|error| {
        error.operation == crate::RuntimeOperation::RuntimeDispatch
            && error
                .message
                .as_deref()
                .is_some_and(|message| message.contains("stop finalize failed"))
    }));
    assert!(runtime.latched_errors().iter().any(|error| {
        error.operation == crate::RuntimeOperation::Recording
            && error
                .message
                .as_deref()
                .is_some_and(|message| message.contains("stop finalize failed"))
    }));

    runner.test_advance_display_time(Duration::from_secs(1));
    runner.messages_with_snapshot().unwrap();
    runner.test_advance_display_time(Duration::from_secs(3));
    let snapshot = runner
        .messages_with_snapshot()
        .unwrap()
        .into_iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot),
            _ => None,
        })
        .unwrap();
    assert_eq!(snapshot["display"]["off"], true);
}
