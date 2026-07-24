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
    assert_eq!(payload["schemaVersion"], 2);
    assert!(payload["revision"].is_number());

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload.clone()).unwrap();

    let restored_payload = restored.config_payload();
    assert_eq!(restored_payload["kind"], "octessera.config");
    assert_eq!(restored_payload["schemaVersion"], 2);
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
    assert_eq!(migrated["schemaVersion"], 2);
    assert!(migrated["runtimeConfig"].is_object());
}

#[test]
pub(crate) fn versioned_v1_config_and_patch_run_legacy_modulation_migration() {
    let mut config_runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut config = config_runner.config_payload();
    config["schemaVersion"] = json!(1);
    config["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("linkLfos");
    config["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("xy");
    config["runtimeConfig"]["layers"][1]["linkLfo"] = json!({
        "enabled": true,
        "target": { "key": "instruments.0.mixer.volume", "kind": "number", "min": 0, "max": 100, "step": 1 },
        "period": "1/4",
        "depthPct": 33
    });
    config["runtimeConfig"]["layers"][1]["xy"] = json!({
        "x": null,
        "y": { "key": "instruments.0.mixer.panPos", "kind": "number", "min": 0, "max": 32, "step": 1 },
        "xInvert": false,
        "yInvert": true
    });
    config_runner.apply_config_payload(config).unwrap();
    assert_eq!(
        config_runner.config_payload()["runtimeConfig"]["linkLfos"][1]["depthPct"],
        33
    );
    assert_eq!(
        config_runner.config_payload()["runtimeConfig"]["xy"]["y"]["key"],
        "instruments.0.mixer.panPos"
    );

    let mut patch_runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let patch = json!({
        "kind": "octessera.patch",
        "schemaVersion": 1,
        "runtimeConfig": {
            "layers": [{
                "linkLfo": {
                    "enabled": true,
                    "target": { "key": "instruments.0.mixer.volume", "kind": "number", "min": 0, "max": 100, "step": 1 },
                    "period": "1/2",
                    "depthPct": 22
                }
            }]
        }
    });
    patch_runner
        .apply_patch_payload_preserving_device(patch)
        .unwrap();
    assert_eq!(
        patch_runner.config_payload()["runtimeConfig"]["linkLfos"][0]["depthPct"],
        22
    );
}

#[test]
pub(crate) fn v2_full_config_requires_global_lfo_bank_but_patch_omission_preserves_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.link_lfos[0].depth_pct = 61;
    let before = runner.config_payload();
    let mut full = before.clone();
    full["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("linkLfos");
    assert!(runner.apply_config_payload(full).is_err());
    assert_eq!(runner.config_payload(), before);

    runner
        .apply_patch_payload_preserving_device(json!({
            "kind": "octessera.patch",
            "schemaVersion": 2,
            "runtimeConfig": { "masterVolume": 74 }
        }))
        .unwrap();
    assert_eq!(runner.link_lfos[0].depth_pct, 61);
}

#[test]
pub(crate) fn malformed_supplied_global_lfo_bank_is_rejected_without_fallback() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let before = runner.config_payload();
    let mut payload = before.clone();
    payload["runtimeConfig"]["linkLfos"] = json!([]);
    assert!(runner.apply_config_payload(payload).is_err());
    assert_eq!(runner.config_payload(), before);
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
pub(crate) fn rejected_config_does_not_drain_live_held_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.instruments[0].note_behavior = "hold".into();
    runner.sync_engine_runtime_config();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    let mut invalid = runner.config_payload();
    invalid["runtimeConfig"]["layers"][0]["worlds"]["behaviorId"] = json!("unsupported-behavior");

    assert!(runner.apply_config_payload(invalid).is_err());
    assert_eq!(runner.engine.drain_held_notes(usize::MAX).len(), 1);
}

#[test]
pub(crate) fn current_schema_rejects_plausible_out_of_range_values() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["masterVolume"] = json!(101);

    assert_rejected_without_byte_changes(&mut runner, payload);
}

#[test]
pub(crate) fn current_schema_rejects_unknown_enums() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["midi"]["syncMode"] = json!("external-ish");

    assert_rejected_without_byte_changes(&mut runner, payload);
}

#[test]
pub(crate) fn current_schema_rejects_malformed_nested_fields() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"] = json!("broken");

    assert_rejected_without_byte_changes(&mut runner, payload);
}

fn assert_rejected_without_byte_changes(runner: &mut NativeRunner, payload: Value) {
    let before_config = serde_json::to_vec(&runner.config_payload()).unwrap();
    let before_snapshot = serde_json::to_vec(&runner.snapshot().unwrap()).unwrap();

    assert!(runner.apply_config_payload(payload).is_err());

    assert_eq!(
        serde_json::to_vec(&runner.config_payload()).unwrap(),
        before_config
    );
    assert_eq!(
        serde_json::to_vec(&runner.snapshot().unwrap()).unwrap(),
        before_snapshot
    );
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
