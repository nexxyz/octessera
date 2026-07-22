use super::queue::{
    queue_by_priority, retain_runtime_outbox_batch, RETAINED_RUNTIME_OUTBOX_BATCHES,
};
use super::{WorkerCommand, PLAYING_SNAPSHOT_INTERVAL_MS};
use crate::types::{encode_runtime_responses, RuntimeMessagesPayload};
use playback_runtime::{
    HostAdapter, HostMessage, MusicalEvent, PlaybackRuntime, RunnerMessage, RuntimeAdapterError,
    RuntimeConfig, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorMetadata, RuntimeOperation,
    RuntimePlatformRequest, RuntimeRecovery, RuntimeStatus, RuntimeStatusState,
    RuntimeTransportState, SyncSource,
};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::mpsc;

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
    playback
        .ingest_runner_messages(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: json!({ "tick": 1 }),
                },
                RunnerMessage::RuntimeStatus {
                    status: RuntimeStatus {
                        state: RuntimeStatusState::Running,
                        transport: RuntimeTransportState::Playing,
                        current_ppqn_pulse: 1,
                        pending_resync: false,
                        sync_source: SyncSource::Internal,
                        message: None,
                        error: None,
                    },
                },
            ],
            &mut host,
        )
        .unwrap();

    playback.latch_error(RuntimeErrorMetadata {
        domain: RuntimeErrorDomain::Audio,
        code: RuntimeErrorCode::OperationFailed,
        operation: RuntimeOperation::AudioCommand,
        recovery: RuntimeRecovery::RetainLastGood,
        request_id: Some("audio-request".into()),
        revision: Some(9),
        message: Some("queue full".into()),
    });
    let output = playback
        .ingest_runner_messages_with_output(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: json!({ "tick": 2 }),
                },
                RunnerMessage::RuntimeStatus {
                    status: RuntimeStatus {
                        state: RuntimeStatusState::Running,
                        transport: RuntimeTransportState::Playing,
                        current_ppqn_pulse: 2,
                        pending_resync: false,
                        sync_source: SyncSource::Internal,
                        message: None,
                        error: None,
                    },
                },
            ],
            &mut host,
        )
        .unwrap();
    let values = encode_runtime_responses(output.messages).unwrap();

    assert!(values.iter().any(|value| {
        value["type"] == "snapshot"
            && value["snapshot"]["tick"] == 2
            && value["snapshot"]["runtimeError"]["revision"] == 9
    }));
    assert!(values.iter().any(|value| {
        value["type"] == "runtime_status"
            && value["status"]["error"]["operation"] == "audio_command"
            && value["status"]["error"]["requestId"] == "audio-request"
    }));
    assert_eq!(playback.last_good_snapshot(), Some(&json!({ "tick": 2 })));
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
