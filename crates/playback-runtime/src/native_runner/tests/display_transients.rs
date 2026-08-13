use super::*;
use crate::native_runner::display_transients::TransportFlash;
use std::time::{Duration, Instant};

fn snapshot_count(messages: &[RunnerMessage]) -> usize {
    messages
        .iter()
        .filter(|message| matches!(message, RunnerMessage::Snapshot { .. }))
        .count()
}

#[test]
fn request_snapshot_false_emits_visible_transitions_once_and_retrigger_only_extends() {
    let start = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.transients.set_test_now(start);
    runner.pending.suppress_snapshot_response = true;
    runner.display.transients.trigger_event_dot(start);

    let first = runner.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_count(&first), 1);
    assert!(matches!(
        first.last(),
        Some(RunnerMessage::RuntimeStatus { .. })
    ));

    let retrigger_at = start + Duration::from_millis(20);
    runner.display.transients.set_test_now(retrigger_at);
    runner.display.transients.trigger_event_dot(retrigger_at);
    let retrigger = runner.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_count(&retrigger), 0);

    runner
        .display
        .transients
        .set_test_now(start + Duration::from_millis(64));
    let before_expiry = runner.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_count(&before_expiry), 0);

    runner
        .display
        .transients
        .set_test_now(start + Duration::from_millis(65));
    let expiry = runner.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_count(&expiry), 1);
}

#[test]
fn flash_kind_change_and_batched_measure_priority_share_one_snapshot() {
    let start = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.transients.set_test_now(start);
    runner.pending.suppress_snapshot_response = true;
    runner
        .display
        .transients
        .trigger_transport_flash(TransportFlash::Beat, start);
    assert_eq!(snapshot_count(&runner.messages_with_snapshot().unwrap()), 1);

    let measure_at = start + Duration::from_millis(10);
    runner.display.transients.set_test_now(measure_at);
    runner
        .display
        .transients
        .trigger_transport_flash(TransportFlash::Measure, measure_at);
    let changed = runner.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_count(&changed), 1);
    assert_eq!(snapshot_from(&changed)["transportFlash"], "measure");
}

#[test]
fn request_snapshot_false_transport_transitions_are_side_effect_free() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.modulation_process_calls = 0;

    let beat = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(snapshot_count(&beat), 1);
    assert_eq!(snapshot_from(&beat)["transportFlash"], "beat");
    assert!(!contains_full_audio_config_command(&beat));
    assert_eq!(runner.modulation_process_calls, 0);

    let measure = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 72,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(snapshot_count(&measure), 1);
    assert_eq!(snapshot_from(&measure)["transportFlash"], "measure");
    assert!(!contains_full_audio_config_command(&measure));
    assert_eq!(runner.modulation_process_calls, 0);
}

#[test]
fn ordinary_request_snapshot_false_transport_steps_do_not_emit_snapshots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;

    for pulses in [1, 1, 6] {
        let messages = runner
            .send(HostMessage::TransportPulseStep {
                pulses,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
        assert_eq!(snapshot_count(&messages), 0);
    }
}

#[test]
fn pending_snapshot_is_retained_until_snapshot_build_succeeds() {
    for suppress_snapshot_response in [false, true] {
        let start = Instant::now();
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.skip_startup_splash();
        runner.display.transients.set_test_now(start);
        runner.pending.suppress_snapshot_response = suppress_snapshot_response;
        runner.display.transients.trigger_event_dot(start);
        runner.test_fail_next_snapshot();

        assert!(runner.messages_with_snapshot().is_err());
        assert!(runner.display.transients.snapshot_pending());

        let retry = runner.messages_with_snapshot().unwrap();
        assert_eq!(snapshot_count(&retry), 1);
        assert!(!runner.display.transients.snapshot_pending());
    }
}

#[test]
fn stop_reset_clears_active_transients_without_tick_snapshots() {
    let start = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.transients.set_test_now(start);
    runner.pending.suppress_snapshot_response = true;
    runner
        .display
        .transients
        .trigger_transport_flash(TransportFlash::Measure, start);
    assert_eq!(snapshot_count(&runner.messages_with_snapshot().unwrap()), 1);

    runner.reset_transport_position();
    let stopped = runner.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_count(&stopped), 1);
    assert_eq!(snapshot_from(&stopped)["transportFlash"], "none");

    let tick = runner.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_count(&tick), 0);
}

fn contains_full_audio_config_command(messages: &[RunnerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            RunnerMessage::AudioCommands { commands }
                if commands
                    .iter()
                    .any(|command| matches!(command, RuntimeAudioCommand::SetAudioConfig { .. }))
        )
    })
}
