use super::*;
use crate::{RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation};
use std::time::{Duration, Instant};

fn send_recording_status(runner: &mut NativeRunner, message: &str) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RecordingStatus {
                ok: true,
                message: message.into(),
                active: message != "Recording saved",
            },
        })
        .unwrap()
}

#[test]
fn recording_statuses_drive_the_presented_recording_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let presented = |runner: &NativeRunner| {
        runner
            .capture_presentation_scene(false)
            .unwrap()
            .oled_presentation_input(crate::oled_frame::OledPresentationMetrics::default(), None)
            .recording_active
    };
    assert!(!presented(&runner));

    let started = send_recording_status(&mut runner, "Recording started");
    assert_eq!(snapshot_from(&started)["recordingActive"], true);
    assert!(presented(&runner));

    send_recording_status(&mut runner, "Recording saved");
    assert!(!presented(&runner));

    send_recording_status(&mut runner, "Recording started");
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure {
                error: RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Recording,
                    crate::RuntimeErrorCode::OperationFailed,
                    RuntimeOperation::Recording,
                    Some("disk full".into()),
                ),
            },
        })
        .unwrap();
    assert!(!presented(&runner));
}

#[test]
fn manual_recording_start_and_duplicate_are_active_native_statuses() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();

    let started = send_recording_status(&mut runner, "Recording started");
    assert_eq!(
        snapshot_from(&started)["display"]["toast"],
        "Recording started"
    );

    let duplicate = send_recording_status(&mut runner, "Recording is already running");
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
fn active_recording_follows_the_oled_sleep_timer() {
    let start = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.test_set_display_time(start);
    runner.display.ui.screen_sleep_seconds = 1;
    runner.display.ui.dim_timer_seconds = 0;
    runner.display.last_interaction_at = start;

    send_recording_status(&mut runner, "Recording started");
    runner.display.toast = None;
    runner.display.toast_expires_at = None;
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(start + Duration::from_millis(33)),
        "the toast slides in first"
    );
    assert_eq!(
        runner.next_timed_display_snapshot_deadline_after(Some(start + Duration::from_millis(99))),
        Some(start + Duration::from_millis(500))
    );
    runner.test_advance_display_time(Duration::from_millis(600));
    assert_eq!(
        runner.next_timed_display_snapshot_deadline_after(Some(start + Duration::from_millis(99))),
        Some(start + Duration::from_secs(1))
    );
    runner.test_advance_display_time(Duration::from_millis(4_400));
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["display"]["splash"], "sleep");
    let next_phase = start + Duration::from_millis(5_500);
    assert_ne!(
        runner.next_timed_display_snapshot_deadline_after(Some(start + Duration::from_secs(5))),
        Some(next_phase),
        "a sleeping screen has no pulse deadline"
    );
    runner.test_advance_display_time(Duration::from_millis(600));
    assert_eq!(
        runner.pending_display_scene_generation(),
        None,
        "nor is it redrawn for the pulse"
    );
}

#[test]
fn recording_stop_and_max_time_completion_show_their_toasts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();

    send_recording_status(&mut runner, "Recording started");
    let stopped = send_recording_status(&mut runner, "Recording saved");
    assert_eq!(
        snapshot_from(&stopped)["display"]["toast"],
        "Recording saved"
    );

    send_recording_status(&mut runner, "Recording started");
    let automatic = send_recording_status(&mut runner, "Max time: saved");
    assert_eq!(
        snapshot_from(&automatic)["display"]["toast"],
        "Max time: saved"
    );
}

#[test]
fn failed_automatic_completion_keeps_error_detail() {
    let start = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.test_set_display_time(start);
    runner.display.ui.screen_sleep_seconds = 1;
    runner.display.ui.dim_timer_seconds = 0;
    runner.display.last_interaction_at = start;

    send_recording_status(&mut runner, "Recording started");
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

    let snapshot = snapshot_from(&failed);
    assert_eq!(snapshot["display"]["title"], "RUNTIME ERROR");
    assert!(snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().is_some_and(|line| line.contains("auto"))));
}
