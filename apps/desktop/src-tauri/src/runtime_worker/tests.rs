use super::queue::{
    queue_by_priority, retain_runtime_outbox_batch, RETAINED_RUNTIME_OUTBOX_BATCHES,
};
use super::{
    observe_accepted_snapshot_revision, periodic_snapshot_due, timed_display_snapshot_due,
    WorkerCommand, PLAYING_SNAPSHOT_INTERVAL_MS,
};
use crate::types::{encode_runtime_responses, RuntimeMessagesPayload};
use playback_runtime::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
    PlaybackRuntime, RunnerMessage, RuntimeAdapterError, RuntimeConfig, RuntimeErrorCode,
    RuntimeErrorDomain, RuntimeErrorMetadata, RuntimeOperation, RuntimePlatformRequest,
    RuntimeRecovery, SyncSource,
};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn runtime_outbox_retains_capped_monotonic_tail() {
    let mut outbox = Vec::new();
    for seq in 1..=RETAINED_RUNTIME_OUTBOX_BATCHES as u64 + 2 {
        retain_runtime_outbox_batch(
            &mut outbox,
            RuntimeMessagesPayload {
                seq,
                messages: vec![serde_json::json!(seq)],
            },
        );
    }

    assert_eq!(outbox.len(), RETAINED_RUNTIME_OUTBOX_BATCHES);
    assert_eq!(outbox.first().map(|payload| payload.seq), Some(3));
    assert_eq!(
        outbox.last().map(|payload| payload.seq),
        Some(RETAINED_RUNTIME_OUTBOX_BATCHES as u64 + 2)
    );
}

#[test]
fn worker_command_priority_separates_midi_realtime_from_normal_work() {
    let (dispatch_tx, _) = mpsc::channel();
    let mut realtime = Vec::new();
    let mut normal = VecDeque::new();

    queue_by_priority(
        WorkerCommand::Dispatch(playback_runtime::HostMessage::MidiRealtimeStop, dispatch_tx),
        &mut realtime,
        &mut normal,
    );
    queue_by_priority(
        WorkerCommand::NativeMidiRealtime(vec![0xF8]),
        &mut realtime,
        &mut normal,
    );

    assert_eq!(realtime.len(), 1);
    assert_eq!(normal.len(), 1);
    assert!(matches!(realtime[0], WorkerCommand::NativeMidiRealtime(_)));
    assert!(matches!(normal[0], WorkerCommand::Dispatch(_, _)));
}

#[test]
fn playing_snapshot_interval_is_coalesced_beyond_frame_rate() {
    let interval_ms = PLAYING_SNAPSHOT_INTERVAL_MS;
    let refresh_ms = crate::types::RUNTIME_UI_REFRESH_MS;
    assert!(interval_ms > 16);
    assert!(interval_ms <= refresh_ms);
}

#[test]
fn accepted_snapshot_resets_periodic_cadence_but_preserves_one_native_deadline() {
    let start = Instant::now();
    let mut cadence_at = start;
    let mut observed_revision = 0;
    observe_accepted_snapshot_revision(
        start + Duration::from_millis(10),
        &mut cadence_at,
        &mut observed_revision,
        1,
    );
    assert!(!periodic_snapshot_due(
        start + Duration::from_millis(59),
        cadence_at,
        Duration::from_millis(50)
    ));
    assert!(periodic_snapshot_due(
        start + Duration::from_millis(60),
        cadence_at,
        Duration::from_millis(50)
    ));

    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    let deadline = runner
        .next_timed_display_snapshot_deadline_after(Some(cadence_at))
        .expect("start should create a native display deadline");
    assert!(!timed_display_snapshot_due(
        deadline - Duration::from_nanos(1),
        cadence_at,
        &runner
    ));
    assert!(timed_display_snapshot_due(deadline, cadence_at, &runner));

    observe_accepted_snapshot_revision(deadline, &mut cadence_at, &mut observed_revision, 2);
    assert!(!timed_display_snapshot_due(deadline, cadence_at, &runner));
}

#[test]
fn failed_native_expiry_does_not_advance_desktop_cadence_until_retry() {
    let start = Instant::now();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = WorkerTestHost::default();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.test_set_display_time(start);
    playback
        .dispatch_host_message(HostMessage::MidiRealtimeStart, &mut runner, &mut host)
        .unwrap();

    let mut cadence_at = start;
    let mut observed_revision = 0;
    observe_accepted_snapshot_revision(
        start,
        &mut cadence_at,
        &mut observed_revision,
        playback.last_snapshot_revision(),
    );
    let deadline = runner
        .next_timed_display_snapshot_deadline_after(Some(cadence_at))
        .expect("start should create a native display deadline");
    runner.test_set_display_time(deadline);
    let accepted_revision_before_failure = playback.last_snapshot_revision();
    runner.test_fail_next_snapshot();
    let expiry = HostMessage::TransportPulseStep {
        pulses: 0,
        source: SyncSource::Internal,
        at_ppqn_pulse: None,
        request_snapshot: Some(false),
    };

    assert!(runner.send(expiry.clone()).is_err());
    observe_accepted_snapshot_revision(
        deadline,
        &mut cadence_at,
        &mut observed_revision,
        playback.last_snapshot_revision(),
    );
    assert_eq!(
        playback.last_snapshot_revision(),
        accepted_revision_before_failure
    );
    assert!(timed_display_snapshot_due(deadline, cadence_at, &runner));

    let retry = runner.send(expiry).unwrap();
    let snapshot_count = retry
        .iter()
        .filter(|message| matches!(message, RunnerMessage::Snapshot { .. }))
        .count();
    assert_eq!(snapshot_count, 1);
    playback.ingest_runner_messages(retry, &mut host).unwrap();
    observe_accepted_snapshot_revision(
        deadline,
        &mut cadence_at,
        &mut observed_revision,
        playback.last_snapshot_revision(),
    );
    assert!(playback.last_snapshot_revision() > accepted_revision_before_failure);
    assert!(!timed_display_snapshot_due(deadline, cadence_at, &runner));
}

#[derive(Default)]
struct WorkerTestHost {
    midi_messages: Vec<Vec<u8>>,
}

impl HostAdapter for WorkerTestHost {
    fn handle_musical_event(&mut self, _event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        _request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        Ok(Vec::new())
    }

    fn handle_audio_command(
        &mut self,
        _command: &playback_runtime::RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        self.midi_messages.push(bytes.to_vec());
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.handle_midi_message(&[0xFC])?;
        for channel in 0..16_u8 {
            self.handle_midi_message(&[0xB0 | channel, 120, 0])?;
            self.handle_midi_message(&[0xB0 | channel, 123, 0])?;
        }
        Ok(())
    }
}

#[test]
fn worker_emits_typed_fault_over_fresh_trusted_snapshot() {
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = WorkerTestHost::default();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let seed = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    playback.ingest_runner_messages(seed, &mut host).unwrap();

    playback.latch_error(RuntimeErrorMetadata {
        domain: RuntimeErrorDomain::Audio,
        code: RuntimeErrorCode::OperationFailed,
        operation: RuntimeOperation::AudioCommand,
        recovery: RuntimeRecovery::RetainLastGood,
        request_id: Some("audio-request".into()),
        revision: Some(9),
        message: Some("queue full".into()),
    });
    let fresh = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    let output = playback
        .ingest_runner_messages_with_output(fresh, &mut host)
        .unwrap();
    let values = encode_runtime_responses(output.messages).unwrap();

    let frame_index = values
        .iter()
        .position(|value| value["type"] == "oled_frame")
        .expect("fault output should include an OLED frame");
    let snapshot_index = values
        .iter()
        .position(|value| value["type"] == "snapshot")
        .expect("fault output should include a snapshot");
    assert!(frame_index < snapshot_index);
    let frame_revision = values[frame_index]["revision"]
        .as_u64()
        .expect("OLED frame revision should be positive");
    assert!(frame_revision > 0);
    assert_eq!(
        values[snapshot_index]["snapshot"]["oledFrameRevision"],
        frame_revision
    );
    assert_eq!(
        values[snapshot_index]["snapshot"]["runtimeError"]["revision"],
        9
    );
    assert!(values.iter().any(|value| {
        value["type"] == "runtime_status"
            && value["status"]["error"]["operation"] == "audio_command"
            && value["status"]["error"]["requestId"] == "audio-request"
    }));
    assert!(playback.last_good_snapshot().is_some());
}

#[test]
fn worker_rejects_non_object_snapshot_without_panic_or_raw_output() {
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = WorkerTestHost::default();
    let output = playback
        .ingest_runner_messages_with_output(
            vec![RunnerMessage::Snapshot {
                snapshot: json!(false),
            }],
            &mut host,
        )
        .unwrap();
    let values = encode_runtime_responses(output.messages).unwrap();

    assert!(values.iter().all(|value| value["type"] != "audio_error"));
    assert!(output
        .follow_ups
        .iter()
        .any(|message| matches!(message, HostMessage::TransportStop)));
    assert_eq!(host.midi_messages.len(), 33);
    assert!(playback.last_good_snapshot().is_none());
}
