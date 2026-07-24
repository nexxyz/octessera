use super::support::{set_runtime_playing, FakeHost, FakeRunner};
use crate::{
    HostMessage, PlaybackRuntime, RunnerMessage, RuntimeConfig, RuntimeErrorCode,
    RuntimeErrorDomain, RuntimeErrorFacts, RuntimeErrorMetadata, RuntimeOperation, RuntimeRecovery,
    RuntimeStatus, RuntimeStatusState, RuntimeStoreResult, RuntimeTransportState, SyncSource,
};
use serde_json::json;

#[test]
fn panic_clears_pending_notes_and_sends_all_notes_off() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: true,
    });
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();

    runtime.advance(500, &mut runner, &mut host).unwrap();
    host.midi_messages.clear();

    runtime.panic(&mut host).unwrap();

    assert_eq!(host.midi_messages.first(), Some(&vec![0xFC]));
    assert_eq!(host.midi_messages.len(), 33);
    assert_eq!(host.silence_calls, 0);
}

#[test]
fn runtime_errors_decorate_presentations_and_preserve_last_good_state() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let good_snapshot = json!({ "transport": { "tick": 1 } });
    let later_snapshot = json!({ "transport": { "tick": 2 } });
    let good_status = RuntimeStatus {
        state: RuntimeStatusState::Running,
        transport: RuntimeTransportState::Playing,
        current_ppqn_pulse: 24,
        pending_resync: false,
        sync_source: SyncSource::Internal,
        message: None,
        error: None,
    };

    runtime
        .ingest_runner_messages(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: good_snapshot.clone(),
                },
                RunnerMessage::RuntimeStatus {
                    status: good_status.clone(),
                },
            ],
            &mut host,
        )
        .unwrap();

    let error = RuntimeErrorMetadata {
        domain: RuntimeErrorDomain::Storage,
        code: RuntimeErrorCode::OperationFailed,
        operation: RuntimeOperation::StoreLoadDefault,
        recovery: RuntimeRecovery::RetainLastGood,
        request_id: Some("load-default-1".into()),
        revision: Some(4),
        message: Some("disk full".into()),
    };
    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::RuntimeStatus {
                status: RuntimeStatus {
                    state: RuntimeStatusState::Error,
                    transport: RuntimeTransportState::Playing,
                    current_ppqn_pulse: 24,
                    pending_resync: false,
                    sync_source: SyncSource::Internal,
                    message: Some("disk full".into()),
                    error: Some(error.clone()),
                },
            }],
            &mut host,
        )
        .unwrap();

    assert_eq!(runtime.last_good_snapshot(), Some(&good_snapshot));
    assert_eq!(runtime.last_good_status(), Some(&good_status));
    assert_eq!(
        runtime.last_status().unwrap().state,
        RuntimeStatusState::Error
    );
    assert_eq!(runtime.last_status().unwrap().error, Some(error.clone()));
    assert_eq!(
        runtime.last_snapshot(),
        Some(&json!({
            "transport": { "tick": 1 },
            "runtimeError": {
                "domain": "storage",
                "code": "operation_failed",
                "operation": "store_load_default",
                "recovery": "retain_last_good",
                "requestId": "load-default-1",
                "revision": 4,
                "message": "disk full"
            }
        }))
    );

    runtime
        .ingest_runner_messages(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: later_snapshot.clone(),
                },
                RunnerMessage::RuntimeStatus {
                    status: good_status,
                },
            ],
            &mut host,
        )
        .unwrap();
    assert_eq!(runtime.last_good_snapshot(), Some(&later_snapshot));
    assert_eq!(
        runtime.last_snapshot(),
        Some(&json!({
            "transport": { "tick": 2 },
            "runtimeError": {
                "domain": "storage",
                "code": "operation_failed",
                "operation": "store_load_default",
                "recovery": "retain_last_good",
                "requestId": "load-default-1",
                "revision": 4,
                "message": "disk full"
            }
        }))
    );

    runtime.ingest_runtime_result(&RuntimeStoreResult::OperationSucceeded {
        operation: RuntimeOperation::StoreLoadDefault,
        request_id: Some("load-default-1".into()),
        revision: Some(4),
    });
    assert!(runtime.latched_errors().is_empty());
    assert_eq!(
        runtime.last_status().unwrap().state,
        RuntimeStatusState::Running
    );
    assert_eq!(runtime.last_snapshot(), Some(&later_snapshot));

    runtime.ingest_runtime_result(&RuntimeStoreResult::SampleListError {
        instrument_slot: 0,
        sample_slot: 0,
        dir: "samples".into(),
        message: "permission denied".into(),
    });
    assert_eq!(
        runtime.latched_errors()[0].operation,
        RuntimeOperation::SampleList
    );
    runtime.ingest_runtime_result(&RuntimeStoreResult::SampleListResult {
        instrument_slot: 0,
        sample_slot: 0,
        dir: "samples".into(),
        entries: Vec::new(),
    });
    assert!(runtime.latched_errors().is_empty());
}

#[test]
fn stop_and_silence_stops_runner_and_panics_all_routes() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();
    set_runtime_playing(&mut runtime, &mut host);
    host.midi_messages.clear();

    runtime
        .recover_from_error(
            RuntimeErrorMetadata::operation_failed(
                RuntimeErrorDomain::Runtime,
                RuntimeOperation::RuntimeDispatch,
                RuntimeRecovery::StopAndSilence,
                "runner failed".into(),
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(matches!(
        runner.seen.last(),
        Some(HostMessage::TransportStop)
    ));
    assert_eq!(host.midi_messages.len(), 34);
    assert_eq!(host.silence_calls, 1);
    assert_eq!(
        runtime.last_status().unwrap().transport,
        RuntimeTransportState::Stopped
    );
    assert_eq!(
        runtime.last_status().unwrap().state,
        RuntimeStatusState::Error
    );
}

#[test]
fn safety_operation_continues_to_external_midi_when_internal_silence_fails() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = FakeRunner::default();
    let mut host = FakeHost {
        fail_internal_silence: true,
        ..FakeHost::default()
    };
    set_runtime_playing(&mut runtime, &mut host);
    host.midi_messages.clear();

    runtime
        .recover_from_error(
            RuntimeErrorMetadata::operation_failed(
                RuntimeErrorDomain::Runtime,
                RuntimeOperation::RuntimeDispatch,
                RuntimeRecovery::StopAndSilence,
                "runner failed".into(),
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(runner
        .seen
        .iter()
        .any(|message| matches!(message, HostMessage::TransportStop)));
    assert_eq!(host.silence_calls, 1);
    assert_eq!(host.midi_messages.len(), 34);
    assert!(runtime
        .latched_errors()
        .iter()
        .any(|error| error.domain == RuntimeErrorDomain::Audio));
}

#[test]
fn malformed_snapshot_is_typed_safe_and_requests_stop_without_panicking() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::Snapshot {
                snapshot: json!({ "trusted": true }),
            }],
            &mut host,
        )
        .unwrap();
    host.midi_messages.clear();

    let output = runtime
        .ingest_runner_messages_with_output(
            vec![RunnerMessage::Snapshot {
                snapshot: json!(42),
            }],
            &mut host,
        )
        .unwrap();

    assert_eq!(
        runtime.last_good_snapshot(),
        Some(&json!({ "trusted": true }))
    );
    assert_eq!(
        runtime.latched_errors()[0].operation,
        RuntimeOperation::Snapshot
    );
    assert_eq!(
        runtime.latched_errors()[0].domain,
        RuntimeErrorDomain::Serialization
    );
    assert_eq!(host.midi_messages.len(), 33);
    assert!(output
        .follow_ups
        .iter()
        .any(|message| matches!(message, HostMessage::TransportStop)));
}

#[test]
fn fault_clear_requires_matching_request_and_revision_identity() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let make_error = |revision| {
        RuntimeErrorMetadata::operation_failed(
            RuntimeErrorDomain::Audio,
            RuntimeOperation::AudioCommand,
            RuntimeRecovery::RetainLastGood,
            "audio queue failed".into(),
        )
        .with_identity(Some("audio-request".into()), Some(revision))
    };

    runtime.latch_error(make_error(1));
    runtime.latch_error(make_error(2));
    runtime.clear_error_with_identity(
        RuntimeOperation::AudioCommand,
        Some("audio-request"),
        Some(1),
    );

    assert_eq!(runtime.latched_errors().len(), 1);
    assert_eq!(runtime.latched_errors()[0].revision, Some(2));
    runtime.ingest_runtime_result(&RuntimeStoreResult::OperationSucceeded {
        operation: RuntimeOperation::AudioCommand,
        request_id: Some("audio-request".into()),
        revision: Some(2),
    });
    assert!(runtime.latched_errors().is_empty());
}

#[test]
fn identified_audio_prep_failure_retains_last_good_snapshot() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let snapshot = json!({ "audio": { "revision": 3 } });
    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::Snapshot {
                snapshot: snapshot.clone(),
            }],
            &mut host,
        )
        .unwrap();

    runtime.ingest_runtime_result(&RuntimeStoreResult::Identified {
        result: Box::new(RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Audio,
                RuntimeErrorCode::InvalidPayload,
                RuntimeOperation::AudioCommand,
                Some("sample decode failed".into()),
            ),
        }),
        request_id: "audio-3".into(),
        revision: Some(3),
    });

    assert_eq!(runtime.last_good_snapshot(), Some(&snapshot));
    assert_eq!(
        runtime.latched_errors()[0].operation,
        RuntimeOperation::AudioCommand
    );
    assert_eq!(
        runtime.latched_errors()[0].code,
        RuntimeErrorCode::InvalidPayload
    );
    assert_eq!(
        runtime.latched_errors()[0].recovery,
        RuntimeRecovery::RetainLastGood
    );
}

#[test]
fn worker_emission_and_persistence_faults_do_not_safety_stop() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = FakeRunner::default();
    let mut host = FakeHost::default();

    runtime
        .recover_from_facts(
            RuntimeErrorFacts::new(
                RuntimeErrorDomain::Runtime,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::RuntimeEmission,
                Some("emit failed".into()),
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(runner.seen.is_empty());
    assert_eq!(
        runtime.latched_errors()[0].recovery,
        RuntimeRecovery::RetainLastGood
    );

    runtime.clear_error(RuntimeOperation::RuntimeEmission);
    runtime
        .recover_from_facts(
            RuntimeErrorFacts::new(
                RuntimeErrorDomain::Storage,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::Persistence,
                Some("save failed".into()),
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(runner.seen.is_empty());
    assert_eq!(runtime.latched_errors()[0].recovery, RuntimeRecovery::Retry);
}

#[test]
fn matching_emission_and_persistence_successes_clear_recovery_faults() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());

    runtime.latch_error(RuntimeErrorMetadata::operation_failed(
        RuntimeErrorDomain::Runtime,
        RuntimeOperation::RuntimeEmission,
        RuntimeRecovery::RetainLastGood,
        "emit failed".into(),
    ));
    runtime.ingest_runtime_result(&RuntimeStoreResult::OperationSucceeded {
        operation: RuntimeOperation::RuntimeEmission,
        request_id: None,
        revision: None,
    });
    assert!(runtime.latched_errors().is_empty());

    runtime.latch_error(RuntimeErrorMetadata::operation_failed(
        RuntimeErrorDomain::Storage,
        RuntimeOperation::Persistence,
        RuntimeRecovery::Retry,
        "save failed".into(),
    ));
    runtime.ingest_runtime_result(&RuntimeStoreResult::Identified {
        result: Box::new(RuntimeStoreResult::SaveRecoveryResult { ok: true }),
        request_id: "recovery-1".into(),
        revision: Some(7),
    });
    assert!(runtime.latched_errors().is_empty());
}

#[test]
fn device_update_failure_retains_playback_without_stop_and_silence() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let result = RuntimeStoreResult::DeviceUpdateStatus {
        ok: false,
        message: "health validation failed".into(),
    };

    runtime.ingest_runtime_result(&result);

    let error = runtime.latched_errors().last().unwrap();
    assert_eq!(error.operation, RuntimeOperation::DeviceUpdate);
    assert_eq!(error.recovery, RuntimeRecovery::RetainLastGood);
    assert_ne!(error.operation, RuntimeOperation::RuntimeDispatch);
    assert_ne!(error.recovery, RuntimeRecovery::StopAndSilence);
}
