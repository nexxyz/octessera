use super::*;

#[test]
pub(crate) fn config_envelope_round_trips_without_reinterpreting_state() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    source
        .apply_config_payload(json!({
            "runtimeConfig": {
                "activeLayerIndex": 1,
                "layers": [
                    {},
                    { "worlds": { "behaviorId": "sequencer" } }
                ],
                "masterVolume": 88
            }
        }))
        .unwrap();
    let payload = source.config_payload();

    assert_eq!(payload["kind"], "octessera.config");
    assert_eq!(payload["schemaVersion"], 1);
    assert!(payload["revision"].is_number());

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload.clone()).unwrap();

    let restored_payload = restored.config_payload();
    assert_eq!(restored_payload["kind"], "octessera.config");
    assert_eq!(restored_payload["schemaVersion"], 1);
    assert_eq!(
        restored_payload["runtimeConfig"]["activeLayerIndex"],
        payload["runtimeConfig"]["activeLayerIndex"]
    );
    assert_eq!(
        restored_payload["runtimeConfig"]["masterVolume"],
        payload["runtimeConfig"]["masterVolume"]
    );
    assert_eq!(restored.behavior.id(), "sequencer");
    assert_eq!(restored.display.ui.master_volume, 88);
}

#[test]
pub(crate) fn legacy_config_is_migrated_to_current_envelope() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut legacy = source.config_payload();
    let object = legacy.as_object_mut().unwrap();
    object.remove("kind");
    object.remove("schemaVersion");
    object.remove("revision");

    source.apply_config_payload(legacy).unwrap();
    let migrated = source.config_payload();

    assert_eq!(migrated["kind"], "octessera.config");
    assert_eq!(migrated["schemaVersion"], 1);
    assert!(migrated["runtimeConfig"].is_object());
}

#[test]
pub(crate) fn rejected_candidate_leaves_runtime_state_and_revisions_unchanged() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.menu.state.stack = vec![0, 1];
    runner.menu.state.cursor = 2;
    let before_payload = runner.config_payload();
    let before_snapshot = runner.snapshot().unwrap();
    let before_transport = runner.transport.clone();
    let before_audio_revision = runner.audio_config_revision;
    let mut invalid = before_payload.clone();
    invalid["runtimeConfig"]["layers"][0]["worlds"]["behaviorId"] = json!("unsupported-behavior");

    assert!(runner.apply_config_payload(invalid).is_err());

    assert_eq!(runner.config_payload(), before_payload);
    assert_eq!(runner.snapshot().unwrap(), before_snapshot);
    assert_eq!(runner.transport, before_transport);
    assert_eq!(runner.audio_config_revision, before_audio_revision);
    assert_eq!(runner.menu.state.stack, vec![0, 1]);
    assert_eq!(runner.menu.state.cursor, 2);
}

#[test]
pub(crate) fn huge_integer_is_rejected_before_any_candidate_commit() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let before_payload = runner.config_payload();
    let before_audio_revision = runner.audio_config_revision;
    let mut invalid = before_payload.clone();
    invalid["runtimeConfig"]["masterVolume"] = json!(u64::MAX);

    let error = runner.apply_config_payload(invalid).unwrap_err();

    assert!(error.contains("masterVolume"));
    assert_eq!(runner.config_payload(), before_payload);
    assert_eq!(runner.audio_config_revision, before_audio_revision);
}

#[test]
pub(crate) fn stale_save_ack_does_not_clear_newer_dirty_revision() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.mark_config_dirty();
    let first_revision = runner.config_revision;
    runner.mark_config_dirty();
    let second_revision = runner.config_revision;

    runner
        .apply_store_result(RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            }),
            request_id: "save-1".into(),
            revision: Some(first_revision),
        })
        .unwrap();
    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, Some(second_revision));

    runner
        .apply_store_result(RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            }),
            request_id: "save-2".into(),
            revision: Some(second_revision),
        })
        .unwrap();
    assert!(!runner.config_dirty);
    assert_eq!(runner.dirty_revision, None);
}

#[test]
pub(crate) fn failed_save_ack_keeps_matching_revision_dirty() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.mark_config_dirty();
    let revision = runner.config_revision;

    runner
        .apply_store_result(RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                ok: false,
                is_auto: Some(true),
            }),
            request_id: "save-failed".into(),
            revision: Some(revision),
        })
        .unwrap();

    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, Some(revision));
}
