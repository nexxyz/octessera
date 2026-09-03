use super::oled_runtime_fixtures::{present, snapshot, status};
use super::support::{FakeHost, FakeRunner};
use crate::{PlaybackRuntime, RunnerMessage, RuntimeConfig, RuntimePresentationMetrics};
use serde_json::json;

#[test]
fn invalid_first_presentation_is_status_only_and_recovery_is_positive_revisioned() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let output = runtime
        .ingest_runner_messages_with_output(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: json!({"nonOledState": {"available": true}}),
                },
                RunnerMessage::RuntimeStatus { status: status() },
            ],
            &mut host,
        )
        .unwrap();
    assert!(output
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
    assert_eq!(runtime.oled_frame_revision(), 0);

    let recovered = present(&mut runtime, snapshot("recovered"));
    assert!(recovered.iter().all(|message| match message {
        RunnerMessage::Snapshot { snapshot } => snapshot
            .get("oledFrameRevision")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|revision| revision > 0),
        _ => true,
    }));

    let after_valid_absent = present(&mut runtime, json!({"nonOledState": {"still": true}}));
    assert!(matches!(
        after_valid_absent.as_slice(),
        [
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { status }
        ] if snapshot["oledFrameRevision"].as_u64().is_some_and(|revision| revision > 0)
            && status.error.as_ref().is_some_and(|error| {
                error.recovery == crate::RuntimeRecovery::RetainLastGood
            })
    ));
}

#[test]
fn every_emitted_snapshot_has_a_positive_revision_across_runtime_paths() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let mut runner = FakeRunner::default();

    let first = present(&mut runtime, snapshot("startup"));
    assert_positive_snapshot_revisions(&first);

    let malformed_first = {
        let mut fresh = PlaybackRuntime::new(RuntimeConfig::default());
        present(&mut fresh, json!({"missingPresentation": true}))
    };
    assert_positive_snapshot_revisions(&malformed_first);

    let after_valid_absent = present(&mut runtime, json!({"nonOledState": true}));
    assert_positive_snapshot_revisions(&after_valid_absent);
    let after_valid_malformed = present(
        &mut runtime,
        json!({"display": {"off": false}, "settings": {"displayBrightness": "bad"}}),
    );
    assert_positive_snapshot_revisions(&after_valid_malformed);

    let platform = runtime
        .dispatch_runner_messages(
            vec![RunnerMessage::PlatformEffects {
                effects: vec![crate::RuntimePlatformEffect::StoreListPresets],
            }],
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_positive_snapshot_revisions(&platform.messages);

    let metrics = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        audio_load_ratio: 0.9,
        voice_steal: true,
        ..Default::default()
    });
    assert_positive_snapshot_revisions(&metrics.messages);

    let recovery = runtime
        .recover_from_facts(
            crate::RuntimeErrorFacts::new(
                crate::RuntimeErrorDomain::Storage,
                crate::RuntimeErrorCode::OperationFailed,
                crate::RuntimeOperation::Store,
                Some("retry".into()),
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_positive_snapshot_revisions(&recovery.messages);
}

fn assert_positive_snapshot_revisions(messages: &[RunnerMessage]) {
    assert!(messages.iter().all(|message| match message {
        RunnerMessage::Snapshot { snapshot } => snapshot
            .get("oledFrameRevision")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|revision| revision > 0),
        _ => true,
    }));
}
