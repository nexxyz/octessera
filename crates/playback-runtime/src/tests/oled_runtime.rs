use super::oled_runtime_fixtures::{present, snapshot, status};
use super::support::{FakeHost, FakeRunner};
use crate::protocol::{base64_encode_count, reset_base64_encode_count};
use crate::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage,
    RuntimeConfig, RuntimeDispatchInput, RuntimePresentationMetrics, SyncSource,
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
    metric_snapshot["eventDotLevel"] = json!(4);
    let _ = present(&mut runtime, metric_snapshot);
    assert!(runtime
        .update_presentation_metrics(RuntimePresentationMetrics {
            audio_load_ratio: 0.02,
            ..Default::default()
        })
        .messages
        .is_empty());
    let hot = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        audio_load_ratio: 0.85,
        worker_utilization: Some(0.93),
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
            RunnerMessage::Snapshot { snapshot } => snapshot.get("workerUtilization"),
            _ => None,
        }),
        Some(&json!(0.93_f32))
    );
}

#[test]
fn missed_quantum_flash_preserves_native_transition_independently_of_cpu_warning() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let _ = present(&mut runtime, snapshot("metrics"));

    let missed = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        worker_utilization: Some(0.9),
        missed_quantum_flash: true,
        ..Default::default()
    });
    assert_eq!(
        missed.messages.iter().find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => snapshot.get("missedQuantumFlash"),
            _ => None,
        }),
        Some(&json!(true))
    );

    let cleared = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        worker_utilization: Some(0.9),
        missed_quantum_flash: false,
        ..Default::default()
    });
    assert_eq!(
        cleared.messages.iter().find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => snapshot.get("missedQuantumFlash"),
            _ => None,
        }),
        Some(&json!(false))
    );
}

#[test]
fn aggregate_load_ratio_drives_the_cpu_meter_without_worker_metrics() {
    assert_eq!(
        crate::oled_frame::OledPresentationMetrics::from_status(None, false).worker_utilization,
        None
    );
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let _ = present(&mut runtime, snapshot("metrics"));

    let output = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        audio_load_ratio: 1.0,
        ..Default::default()
    });

    assert!(matches!(
        output.messages.as_slice(),
        [
            RunnerMessage::OledFrame { revision: 2, .. },
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { .. }
        ] if snapshot.get("workerUtilization").is_none()
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
        "settings": { "displayBrightness": 16 },
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
    runner.test_enable_midi();
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
    assert_ordered_revisioned_presentation_with_flash(&start_output.messages, 2, 0, "measure");

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
    assert_ordered_revisioned_presentation_with_flash(&event.messages, 3, 4, "none");

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
    assert_ordered_revisioned_presentation_with_flash(&flash.messages, 4, 4, "beat");

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
    assert_ordered_revisioned_presentation_with_flash(&flash_expiry.messages, 5, 4, "none");

    runner.test_set_display_time(start + std::time::Duration::from_millis(291));
    let event_step = runtime
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
    assert_ordered_revisioned_presentation_with_flash(&event_step.messages, 6, 3, "none");

    runner.test_set_display_time(start + std::time::Duration::from_millis(591));
    let event_faded = runtime
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
    assert_ordered_revisioned_presentation_with_flash(&event_faded.messages, 7, 0, "none");
}

fn assert_ordered_revisioned_presentation_with_flash(
    messages: &[RunnerMessage],
    revision: u64,
    event_dot_level: u8,
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
            && snapshot["eventDotLevel"] == event_dot_level
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
