use super::*;
use crate::{RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation};
use std::time::{Duration, Instant};

fn send_recording_status(
    runner: &mut NativeRunner,
    message: &str,
    active: bool,
) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RecordingStatus {
                ok: true,
                message: message.into(),
                active,
            },
        })
        .unwrap()
}

#[test]
fn manual_recording_start_and_duplicate_are_active_native_statuses() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let started = send_recording_status(&mut runner, "Recording started", true);
    assert!(runner.recording_active);
    assert_eq!(
        snapshot_from(&started)["display"]["toast"],
        "Recording started"
    );

    let duplicate = send_recording_status(&mut runner, "Recording is already running", true);
    assert!(runner.recording_active);
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Recording is already running")
    );
    let _ = duplicate;
}

#[test]
fn active_recording_suppresses_sleep_deadline_and_oled_off_transition() {
    let start = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.test_set_display_time(start);
    runner.display.ui.screen_sleep_seconds = 1;
    runner.display.ui.dim_timer_seconds = 0;
    runner.display.last_interaction_at = start;

    send_recording_status(&mut runner, "Recording started", true);
    runner.display.toast = None;
    runner.display.toast_expires_at = None;
    runner.test_advance_display_time(Duration::from_secs(5));
    assert!(runner.next_timed_display_snapshot_deadline().is_none());
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["display"]["off"], false);
    assert_eq!(snapshot["display"]["splash"], "");
}

#[test]
fn recording_stop_and_max_time_completion_restore_sleep_eligibility() {
    let start = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.test_set_display_time(start);
    runner.display.ui.screen_sleep_seconds = 1;
    runner.display.last_interaction_at = start;
    let configured_sleep_seconds = runner.display.ui.screen_sleep_seconds;

    send_recording_status(&mut runner, "Recording started", true);
    let completed = start + Duration::from_secs(5);
    runner.test_set_display_time(completed);
    let stopped = send_recording_status(&mut runner, "Recording saved", false);
    assert!(!runner.recording_active);
    assert_eq!(runner.display.last_interaction_at, completed);
    assert_eq!(
        runner.display.ui.screen_sleep_seconds,
        configured_sleep_seconds
    );
    assert_eq!(
        snapshot_from(&stopped)["display"]["toast"],
        "Recording saved"
    );

    runner.test_advance_display_time(Duration::from_secs(1));
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["display"]["splash"], "sleep");

    send_recording_status(&mut runner, "Recording started", true);
    runner.test_set_display_time(completed + Duration::from_secs(5));
    let automatic = send_recording_status(&mut runner, "Max time: saved", false);
    assert!(!runner.recording_active);
    assert_eq!(
        snapshot_from(&automatic)["display"]["toast"],
        "Max time: saved"
    );
}

#[test]
fn failed_automatic_completion_clears_recording_lifecycle_and_keeps_error_detail() {
    let start = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.test_set_display_time(start);
    runner.display.ui.screen_sleep_seconds = 1;
    runner.display.ui.dim_timer_seconds = 0;
    runner.display.last_interaction_at = start;

    send_recording_status(&mut runner, "Recording started", true);
    runner.display.toast = None;
    runner.display.toast_expires_at = None;
    runner.test_advance_display_time(Duration::from_secs(5));
    let failed = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure {
                error: RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Recording,
                    RuntimeErrorCode::OperationFailed,
                    RuntimeOperation::Recording,
                    Some("auto finalize failed".into()),
                ),
            },
        })
        .unwrap();

    assert!(!runner.recording_active);
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + Duration::from_secs(6))
    );
    let snapshot = snapshot_from(&failed);
    assert_eq!(snapshot["display"]["title"], "RUNTIME ERROR");
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().is_some_and(|line| line.contains("auto"))));
}

#[test]
fn changed_config_transaction_preserves_active_recording_sleep_suppression() {
    let start = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.test_set_display_time(start);
    runner.display.ui.screen_sleep_seconds = 1;
    runner.display.ui.dim_timer_seconds = 0;
    runner.display.last_interaction_at = start;

    send_recording_status(&mut runner, "Recording started", true);
    runner.display.toast = None;
    runner.display.toast_expires_at = None;
    runner.test_advance_display_time(Duration::from_secs(5));
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["masterVolume"] = json!(81);
    runner.apply_config_payload(payload).unwrap();

    assert!(runner.recording_active);
    assert_eq!(runner.config_payload()["runtimeConfig"]["masterVolume"], 81);
    assert!(runner.next_timed_display_snapshot_deadline().is_none());
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["display"]["off"], false);
    assert_eq!(snapshot["display"]["splash"], "");
}
