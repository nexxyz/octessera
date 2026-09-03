use super::oled_runtime_fixtures::{present, snapshot, status};
use super::support::{FakeHost, FakeRunner};
use crate::protocol::{base64_encode_count, reset_base64_encode_count};
use crate::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage,
    RuntimeConfig, RuntimeDispatchInput, RuntimePresentationMetrics, RuntimeStatusState,
    SyncSource,
};
use base64::Engine;
use serde_json::json;

#[test]
fn first_changed_and_unchanged_presentations_have_ordered_revisioned_frames() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let first = present(&mut runtime, snapshot("first"));
    assert!(matches!(first.as_slice(), [
        RunnerMessage::OledFrame { revision: 1, .. },
        RunnerMessage::Snapshot { snapshot },
        RunnerMessage::RuntimeStatus { .. }
    ] if snapshot["oledFrameRevision"] == 1));

    reset_base64_encode_count();
    let unchanged = present(&mut runtime, snapshot("first"));
    assert!(matches!(unchanged.as_slice(), [
        RunnerMessage::Snapshot { snapshot },
        RunnerMessage::RuntimeStatus { .. }
    ] if snapshot["oledFrameRevision"] == 1));
    let _wire = serde_json::to_string(&unchanged).unwrap();
    assert_eq!(base64_encode_count(), 0);

    let changed = present(&mut runtime, snapshot("second"));
    assert!(matches!(changed.as_slice(), [
        RunnerMessage::OledFrame { revision: 2, .. },
        RunnerMessage::Snapshot { snapshot },
        RunnerMessage::RuntimeStatus { .. }
    ] if snapshot["oledFrameRevision"] == 2));
}

#[test]
fn runtime_error_overlay_is_rendered_after_enrichment() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let _ = present(&mut runtime, snapshot("normal"));
    runtime.latch_facts(crate::RuntimeErrorFacts::new(
        crate::RuntimeErrorDomain::Storage,
        crate::RuntimeErrorCode::OperationFailed,
        crate::RuntimeOperation::Store,
        Some("disk full".into()),
    ));
    let mut host = FakeHost::default();
    let output = runtime
        .recover_from_facts(
            crate::RuntimeErrorFacts::new(
                crate::RuntimeErrorDomain::Storage,
                crate::RuntimeErrorCode::OperationFailed,
                crate::RuntimeOperation::Store,
                Some("disk full".into()),
            ),
            &mut FakeRunner::default(),
            &mut host,
        )
        .unwrap();
    assert!(matches!(output.messages.as_slice(), [
        RunnerMessage::OledFrame { revision: 2, .. },
        RunnerMessage::Snapshot { snapshot },
        RunnerMessage::RuntimeStatus { .. }
    ] if snapshot["runtimeError"]["message"] == "disk full"));
}

#[test]
fn normalized_metrics_only_publish_visible_changes() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut metric_snapshot = snapshot("metrics");
    metric_snapshot["eventDotOn"] = json!(true);
    let _ = present(&mut runtime, metric_snapshot);
    assert!(runtime
        .update_presentation_metrics(RuntimePresentationMetrics {
            audio_load_ratio: 0.99,
            voice_steal: false,
            ..Default::default()
        })
        .messages
        .is_empty());
    let hot = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        audio_load_ratio: 0.85,
        voice_steal: false,
        worker_utilization: Some(0.93),
        high_cpu_steady: true,
        ..Default::default()
    });
    assert!(matches!(
        hot.messages.as_slice(),
        [
            RunnerMessage::OledFrame { revision: 2, .. },
            RunnerMessage::Snapshot { .. },
            RunnerMessage::RuntimeStatus { .. }
        ]
    ));
    assert_eq!(
        hot.messages.iter().find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => snapshot.get("highCpuSteady"),
            _ => None,
        }),
        Some(&json!(true))
    );
    assert_eq!(
        hot.messages.iter().find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => snapshot.get("workerUtilization"),
            _ => None,
        }),
        Some(&json!(0.93_f32))
    );
}

#[test]
fn aggregate_load_ratio_does_not_set_worker_warning_state() {
    assert!(
        !crate::oled_frame::OledPresentationMetrics::from_status(None, true, false, false)
            .high_cpu_steady
    );
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let _ = present(&mut runtime, snapshot("metrics"));

    let output = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        audio_load_ratio: 1.0,
        ..Default::default()
    });

    assert!(output.messages.is_empty());
}

#[test]
fn normalized_voice_steal_metrics_publish_on_edges_and_clear_below_threshold() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut metric_snapshot = snapshot("metrics");
    metric_snapshot["eventDotOn"] = json!(true);
    let _ = present(&mut runtime, metric_snapshot);

    assert!(runtime
        .update_presentation_metrics(RuntimePresentationMetrics {
            audio_load_ratio: 0.84,
            voice_steal: false,
            ..Default::default()
        })
        .messages
        .is_empty());

    let voice_steal = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        audio_load_ratio: 0.2,
        voice_steal: true,
        ..Default::default()
    });
    assert!(matches!(
        voice_steal.messages.as_slice(),
        [
            RunnerMessage::OledFrame { revision: 2, .. },
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { .. }
        ] if snapshot["oledFrameRevision"] == 2
    ));

    let cleared = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        audio_load_ratio: 0.84,
        voice_steal: false,
        ..Default::default()
    });
    assert!(matches!(
        cleared.messages.as_slice(),
        [
            RunnerMessage::OledFrame { revision: 3, .. },
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { .. }
        ] if snapshot["oledFrameRevision"] == 3
    ));
}

#[test]
fn adapter_cache_faults_are_presented_as_typed_retain_last_good_status() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let _ = present(&mut runtime, snapshot("cache fault"));

    let output = runtime.report_oled_cache_fault(Some(crate::RuntimeOledCacheFault::Future));
    assert!(output.messages.iter().all(|message| !matches!(
        message,
        RunnerMessage::OledFrame { .. } | RunnerMessage::Snapshot { .. }
    )));
    assert!(output.follow_ups.is_empty());
    assert!(matches!(
        output.messages.as_slice(),
        [RunnerMessage::RuntimeStatus { status }]
            if status.state == RuntimeStatusState::Error
                && status.error.as_ref().is_some_and(|error| {
                    error.domain == crate::RuntimeErrorDomain::Serialization
                        && error.code == crate::RuntimeErrorCode::InvalidPayload
                        && error.operation == crate::RuntimeOperation::Snapshot
                        && error.recovery == crate::RuntimeRecovery::RetainLastGood
                })
    ));

    let recovered = runtime.report_oled_cache_fault(None);
    assert!(matches!(
        recovered.messages.as_slice(),
        [RunnerMessage::RuntimeStatus { status }]
            if status.state == RuntimeStatusState::Idle && status.error.is_none()
    ));
}

#[test]
fn partial_oled_presentation_fails_closed_without_losing_non_oled_snapshot_state() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let _ = present(&mut runtime, snapshot("valid"));
    let previous_revision = runtime.oled_frame_revision();
    let previous_pixels = runtime.last_oled_frame().unwrap().to_vec();
    let partial = json!({
        "display": { "title": "Partial candidate" },
        "settings": { "displayBrightness": 100 },
        "nonOledState": { "still": "available" }
    });

    let output = present(&mut runtime, partial.clone());
    assert!(output
        .iter()
        .all(|message| !matches!(message, RunnerMessage::OledFrame { .. })));
    assert!(matches!(
        output.as_slice(),
        [
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { status }
        ] if snapshot["display"]["title"] == "Partial candidate"
            && snapshot["nonOledState"]["still"] == "available"
            && snapshot["oledFrameRevision"] == previous_revision
            && status.error.as_ref().is_some_and(|error| error.domain
                == crate::RuntimeErrorDomain::Serialization)
    ));
    assert_eq!(runtime.last_good_snapshot(), Some(&partial));
    assert_eq!(runtime.oled_frame_revision(), previous_revision);
    assert_eq!(runtime.last_oled_frame(), Some(previous_pixels.as_slice()));

    let recovered = present(&mut runtime, snapshot("recovered"));
    assert!(matches!(
        recovered.as_slice(),
        [
            RunnerMessage::OledFrame { revision, .. },
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { status }
        ] if *revision == previous_revision + 1
            && snapshot["oledFrameRevision"] == *revision
            && status.error.is_none()
    ));
}

#[test]
fn coalesced_presentations_publish_only_the_latest_frame_snapshot_pair() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let output = runtime
        .ingest_runner_messages_with_output(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: snapshot("first"),
                },
                RunnerMessage::RuntimeStatus { status: status() },
                RunnerMessage::Snapshot {
                    snapshot: snapshot("second"),
                },
                RunnerMessage::RuntimeStatus { status: status() },
                RunnerMessage::Snapshot {
                    snapshot: snapshot("latest"),
                },
                RunnerMessage::RuntimeStatus { status: status() },
            ],
            &mut host,
        )
        .unwrap();

    assert!(matches!(
        output.messages.as_slice(),
        [
            RunnerMessage::OledFrame { revision, pixels, .. },
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { .. }
        ] if *revision == 3
            && snapshot["display"]["title"] == "latest"
            && snapshot["oledFrameRevision"] == *revision
            && runtime.last_oled_frame() == Some(pixels.as_slice())
    ));
}

#[test]
fn native_transient_transitions_reach_playback_with_matching_oled_revisions() {
    let start = std::time::Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.skip_startup_splash();
    runner.test_set_display_time(start);
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();

    let _initial = runtime
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_eq!(runtime.oled_frame_revision(), 1);

    let start_output = runtime
        .dispatch_runner_messages(
            runner.send(HostMessage::MidiRealtimeStart).unwrap(),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_ordered_revisioned_presentation_with_flash(&start_output.messages, 2, false, "measure");

    runner.test_set_display_time(start + std::time::Duration::from_millis(91));
    let event = runtime
        .dispatch(
            RuntimeDispatchInput::RunnerMessages(
                runner
                    .send(HostMessage::DeviceInput {
                        input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
                        request_snapshot: Some(false),
                    })
                    .unwrap(),
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(!host.musical_events.is_empty());
    assert_ordered_revisioned_presentation_with_flash(&event.messages, 3, true, "none");

    let audio_commands = host.audio_commands.len();
    runner.test_set_display_time(start + std::time::Duration::from_millis(120));
    let flash = runtime
        .dispatch(
            RuntimeDispatchInput::RunnerMessages(
                runner
                    .send(HostMessage::TransportPulseStep {
                        pulses: 24,
                        source: SyncSource::Internal,
                        at_ppqn_pulse: None,
                        request_snapshot: Some(false),
                    })
                    .unwrap(),
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_eq!(host.audio_commands.len(), audio_commands);
    assert_ordered_revisioned_presentation_with_flash(&flash.messages, 4, true, "beat");

    runner.test_set_display_time(start + std::time::Duration::from_millis(140));
    let event_expiry = runtime
        .dispatch(
            RuntimeDispatchInput::RunnerMessages(
                runner
                    .send(HostMessage::TransportPulseStep {
                        pulses: 0,
                        source: SyncSource::Internal,
                        at_ppqn_pulse: None,
                        request_snapshot: Some(false),
                    })
                    .unwrap(),
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_ordered_revisioned_presentation_with_flash(&event_expiry.messages, 5, false, "beat");

    runner.test_set_display_time(start + std::time::Duration::from_millis(220));
    let flash_expiry = runtime
        .dispatch(
            RuntimeDispatchInput::RunnerMessages(
                runner
                    .send(HostMessage::TransportPulseStep {
                        pulses: 0,
                        source: SyncSource::Internal,
                        at_ppqn_pulse: None,
                        request_snapshot: Some(false),
                    })
                    .unwrap(),
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_ordered_revisioned_presentation_with_flash(&flash_expiry.messages, 6, false, "none");
}

fn assert_ordered_revisioned_presentation_with_flash(
    messages: &[RunnerMessage],
    revision: u64,
    event_dot_on: bool,
    transport_flash: &str,
) {
    assert!(matches!(
        messages,
        [
            RunnerMessage::OledFrame { revision: frame_revision, .. },
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { .. }
        ] if *frame_revision == revision
            && snapshot["oledFrameRevision"] == revision
            && snapshot["eventDotOn"] == event_dot_on
            && snapshot["transportFlash"] == transport_flash
    ));
}

#[test]
fn frame_wire_payload_has_exact_size_and_stays_below_limit() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let frame = present(&mut runtime, snapshot("wire"))
        .into_iter()
        .find_map(|message| match message {
            RunnerMessage::OledFrame { .. } => Some(message),
            _ => None,
        })
        .unwrap();
    let wire = serde_json::to_string(&frame).unwrap();
    assert!(wire.len() < 45 * 1024);
    let pixels = match frame {
        RunnerMessage::OledFrame { pixels, .. } => pixels,
        _ => unreachable!(),
    };
    assert_eq!(pixels.len(), 32768);
    let encoded = serde_json::to_value(RunnerMessage::OledFrame {
        revision: 1,
        width: 128,
        height: 128,
        format: "rgb565be".into(),
        pixels,
    })
    .unwrap()["pixelsBase64"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap()
            .len(),
        32768
    );
}
