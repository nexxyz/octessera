use crate::oled_frame::INDICATOR_FADE_LEVELS;
use crate::tests::support::{canonical_oled_snapshot, FakeHost};
use crate::{
    PlaybackRuntime, RunnerMessage, RuntimeConfig, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorMetadata, RuntimeOperation, RuntimePresentationMetrics, RuntimeRecovery,
};

#[test]
fn native_state_reads_normalized_metrics_without_revising_or_serializing_oled() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    runtime
        .ingest_runner_messages_with_output(
            vec![RunnerMessage::Snapshot {
                snapshot: canonical_oled_snapshot("normal"),
            }],
            &mut host,
        )
        .unwrap();
    runtime.update_presentation_metrics(RuntimePresentationMetrics {
        worker_utilization: Some(0.93),
        high_cpu_steady: true,
        missed_quantum_flash: true,
        voice_steal: true,
        ..Default::default()
    });
    let revision = runtime.oled_frame_revision();
    let snapshot_revision = runtime.last_snapshot_revision();
    let snapshot = runtime.last_snapshot().cloned();
    for _ in 0..3 {
        let (metrics, error) = runtime.native_presentation_state();
        assert_eq!(metrics.worker_utilization, Some(0.93));
        assert!(metrics.high_cpu_steady);
        assert!(metrics.missed_quantum_flash);
        assert_eq!(metrics.voice_steal_level, INDICATOR_FADE_LEVELS);
        assert_eq!(error, None);
    }
    assert_eq!(runtime.oled_frame_revision(), revision);
    assert_eq!(runtime.last_snapshot_revision(), snapshot_revision);
    assert_eq!(runtime.last_snapshot(), snapshot.as_ref());
    runtime.update_presentation_metrics(RuntimePresentationMetrics {
        worker_utilization: None,
        high_cpu_steady: true,
        ..Default::default()
    });
    let (metrics, _) = runtime.native_presentation_state();
    assert_eq!(metrics.worker_utilization, None);
    assert!(!metrics.high_cpu_steady);
}

#[test]
fn native_error_matches_wire_names_and_existing_status_priority_through_recovery() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let latched = RuntimeErrorMetadata::new(
        RuntimeErrorDomain::Storage,
        RuntimeErrorCode::OperationFailed,
        RuntimeOperation::StoreSaveDefault,
        RuntimeRecovery::RetainLastGood,
        Some("disk full".into()),
    )
    .with_identity(Some("save-42".into()), Some(7));
    runtime.latch_error(latched.clone());
    let (_, error) = runtime.native_presentation_state();
    let error = error.unwrap();
    assert_eq!(error.domain.as_deref(), Some("storage"));
    assert_eq!(error.code.as_deref(), Some("operation_failed"));
    assert_eq!(error.operation.as_deref(), Some("store_save_default"));
    assert_eq!(error.message.as_deref(), Some("disk full"));
    assert_eq!(
        runtime.last_status().unwrap().error.as_ref(),
        Some(&latched)
    );
    runtime.clear_error_with_identity(RuntimeOperation::StoreSaveDefault, Some("save-42"), Some(7));
    assert_eq!(runtime.native_presentation_state().1, None);
}

#[test]
fn render_fault_is_presented_until_a_valid_snapshot_recovers_it() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    runtime
        .ingest_runner_messages_with_output(
            vec![RunnerMessage::Snapshot {
                snapshot: serde_json::json!({"display": {}}),
            }],
            &mut FakeHost::default(),
        )
        .unwrap();
    let (_, error) = runtime.native_presentation_state();
    let error = error.unwrap();
    assert_eq!(error.domain.as_deref(), Some("serialization"));
    assert_eq!(error.code.as_deref(), Some("invalid_payload"));
    assert_eq!(error.operation.as_deref(), Some("snapshot"));
    assert!(error
        .message
        .unwrap()
        .starts_with("OLED presentation field is invalid:"));
    runtime
        .ingest_runner_messages_with_output(
            vec![RunnerMessage::Snapshot {
                snapshot: canonical_oled_snapshot("recovered"),
            }],
            &mut FakeHost::default(),
        )
        .unwrap();
    assert_eq!(runtime.native_presentation_state().1, None);
}

#[test]
fn typed_error_names_match_canonical_serde_wire_names() {
    for domain in [
        RuntimeErrorDomain::Runtime,
        RuntimeErrorDomain::Storage,
        RuntimeErrorDomain::Midi,
        RuntimeErrorDomain::Sample,
        RuntimeErrorDomain::Audio,
        RuntimeErrorDomain::Serialization,
        RuntimeErrorDomain::Recording,
    ] {
        for code in [
            RuntimeErrorCode::OperationFailed,
            RuntimeErrorCode::Unavailable,
            RuntimeErrorCode::InvalidPayload,
            RuntimeErrorCode::NotFound,
            RuntimeErrorCode::Unsupported,
            RuntimeErrorCode::SerializationFailed,
            RuntimeErrorCode::AudioThreadFailed,
        ] {
            let operation = RuntimeOperation::MidiListInputs;
            let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
            runtime.latch_error(RuntimeErrorMetadata::new(
                domain.clone(),
                code.clone(),
                operation.clone(),
                RuntimeRecovery::RetainLastGood,
                None,
            ));
            let error = runtime.native_presentation_state().1.unwrap();
            assert_eq!(
                error.domain.as_deref(),
                serde_json::to_value(&domain).unwrap().as_str()
            );
            assert_eq!(
                error.code.as_deref(),
                serde_json::to_value(&code).unwrap().as_str()
            );
            assert_eq!(
                error.operation.as_deref(),
                serde_json::to_value(&operation).unwrap().as_str()
            );
            assert!(error.message.is_none());
        }
    }
    for operation in [
        RuntimeOperation::RuntimeDispatch,
        RuntimeOperation::DeviceInput,
        RuntimeOperation::Transport,
        RuntimeOperation::MusicalEvent,
        RuntimeOperation::MidiEvent,
        RuntimeOperation::MidiMessage,
        RuntimeOperation::AudioCommand,
        RuntimeOperation::AudioThread,
        RuntimeOperation::Snapshot,
        RuntimeOperation::TransportStop,
        RuntimeOperation::Store,
        RuntimeOperation::StoreListPresets,
        RuntimeOperation::StoreLoadPreset,
        RuntimeOperation::StoreSavePreset,
        RuntimeOperation::StoreDeletePreset,
        RuntimeOperation::StoreLoadDefault,
        RuntimeOperation::StoreSaveDefault,
        RuntimeOperation::StoreSaveBackup,
        RuntimeOperation::StoreSaveRecovery,
        RuntimeOperation::RuntimeEmission,
        RuntimeOperation::Persistence,
        RuntimeOperation::MidiListOutputs,
        RuntimeOperation::MidiListInputs,
        RuntimeOperation::MidiStatus,
        RuntimeOperation::SampleList,
        RuntimeOperation::SamplePreview,
        RuntimeOperation::DeviceUpdate,
        RuntimeOperation::Recording,
        RuntimeOperation::SystemInfo,
        RuntimeOperation::SetupPortal,
        RuntimeOperation::UserDataTransfer,
    ] {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        runtime.latch_error(RuntimeErrorMetadata::new(
            RuntimeErrorDomain::Runtime,
            RuntimeErrorCode::OperationFailed,
            operation.clone(),
            RuntimeRecovery::RetainLastGood,
            None,
        ));
        let error = runtime.native_presentation_state().1.unwrap();
        assert_eq!(
            error.operation.as_deref(),
            serde_json::to_value(&operation).unwrap().as_str()
        );
    }
}

#[test]
fn native_metric_updates_change_only_typed_state_and_report_normalized_edges() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let first = RuntimePresentationMetrics {
        audio_load_ratio: 0.9,
        worker_utilization: Some(0.91),
        high_cpu_steady: true,
        missed_quantum_flash: true,
        voice_steal: true,
    };
    assert!(runtime.update_native_presentation_metrics(first));
    let (metrics, error) = runtime.native_presentation_state();
    assert_eq!(metrics.worker_utilization, Some(0.91));
    assert!(metrics.high_cpu_steady);
    assert!(metrics.missed_quantum_flash);
    assert_eq!(metrics.voice_steal_level, INDICATOR_FADE_LEVELS);
    assert_eq!(error, None);
    let revision = runtime.oled_frame_revision();
    let snapshot_revision = runtime.last_snapshot_revision();
    let render_count = runtime.test_oled_render_count();
    assert!(
        !runtime.update_native_presentation_metrics(RuntimePresentationMetrics {
            audio_load_ratio: 0.1,
            ..first
        })
    );
    assert_eq!(runtime.oled_frame_revision(), revision);
    assert_eq!(runtime.last_snapshot_revision(), snapshot_revision);
    assert_eq!(runtime.test_oled_render_count(), render_count);

    assert!(
        runtime.update_native_presentation_metrics(RuntimePresentationMetrics {
            worker_utilization: None,
            high_cpu_steady: true,
            missed_quantum_flash: false,
            voice_steal: false,
            ..Default::default()
        })
    );
    let (cleared, _) = runtime.native_presentation_state();
    assert_eq!(cleared.worker_utilization, None);
    assert!(!cleared.high_cpu_steady);
    assert!(!cleared.missed_quantum_flash);
    assert_eq!(cleared.voice_steal_level, INDICATOR_FADE_LEVELS - 1);
    assert_eq!(runtime.oled_frame_revision(), revision);
    assert_eq!(runtime.last_snapshot_revision(), snapshot_revision);
    assert_eq!(runtime.test_oled_render_count(), render_count);
}

#[test]
fn cpu_load_holds_the_one_second_peak_and_voice_steal_fades_per_report() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let report = |worker_utilization: Option<f32>, audio_load_ratio: f32, voice_steal: bool| {
        RuntimePresentationMetrics {
            audio_load_ratio,
            worker_utilization,
            voice_steal,
            ..Default::default()
        }
    };
    runtime.update_native_presentation_metrics(report(Some(0.87), 0.1, true));
    let mut peaks = Vec::new();
    let mut steals = Vec::new();
    for _ in 0..10 {
        runtime.update_native_presentation_metrics(report(Some(0.2), 0.1, false));
        let (metrics, _) = runtime.native_presentation_state();
        peaks.push(metrics.cpu_load_peak_percent);
        steals.push(metrics.voice_steal_level);
    }
    assert_eq!(peaks, [85, 85, 85, 85, 85, 85, 85, 85, 85, 20]);
    assert_eq!(steals, [3, 2, 1, 0, 0, 0, 0, 0, 0, 0]);

    runtime.update_native_presentation_metrics(report(Some(0.2), 0.1, true));
    assert_eq!(
        runtime.native_presentation_state().0.voice_steal_level,
        INDICATOR_FADE_LEVELS
    );
    for _ in 0..10 {
        runtime.update_native_presentation_metrics(report(None, 0.62, false));
    }
    assert_eq!(
        runtime.native_presentation_state().0.cpu_load_peak_percent,
        60
    );
}
