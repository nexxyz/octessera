use super::*;

#[test]
pub(crate) fn no_op_config_edit_produces_no_runtime_plan_or_audio_revision() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.messages_with_snapshot().unwrap();
    let payload = runner.config_payload();
    let before = runner.configuration_aggregate();
    let before_revision = runner.audio_config_revision;
    let before_config_revision = runner.config_revision;

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(
        before.resolve_plan(&runner.configuration_aggregate(), before_revision),
        ConfigurationRuntimePlan::NoRuntimeChange
    );
    assert_eq!(runner.audio_config_revision, before_revision);
    assert_eq!(runner.config_revision, before_config_revision);
    let messages = runner.messages_with_snapshot().unwrap();
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands
                .iter()
                .any(|command| matches!(command, RuntimeAudioCommand::SetAudioConfig { .. }))
    )));
}

#[test]
pub(crate) fn one_config_transaction_produces_one_full_revisioned_update() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.messages_with_snapshot().unwrap();
    let before = runner.configuration_aggregate();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["masterVolume"] = json!(81);
    payload["runtimeConfig"]["instruments"][0]["synth"]["amp"]["gainPct"] = json!(71);

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(
        before.resolve_plan(&runner.configuration_aggregate(), 0),
        ConfigurationRuntimePlan::FullRevisionedConfiguration { revision: 1 }
    );
    assert_eq!(runner.audio_config_revision, 1);
    let messages = runner.messages_with_snapshot().unwrap();
    let full_updates = messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => Some(commands),
            _ => None,
        })
        .flatten()
        .filter(|command| matches!(command, RuntimeAudioCommand::SetAudioConfig { .. }))
        .count();
    assert_eq!(full_updates, 1);
}

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
pub(crate) fn canonical_audio_outputs_survive_default_load_save_reload() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut default_payload = source.config_payload();
    default_payload["runtimeConfig"]["audioOutputs"] = json!({
        "dac": false,
        "usb": true,
        "hdmi": false
    });
    default_payload["runtimeConfig"]["usb"]
        .as_object_mut()
        .unwrap()
        .remove("audioOut");

    source
        .apply_store_result(RuntimeStoreResult::LoadDefaultResult {
            payload: Some(default_payload),
        })
        .unwrap();
    let RuntimePlatformEffect::StoreSaveDefault { payload, .. } = source
        .platform_effect_for_action("default.save")
        .unwrap()
        .unwrap()
    else {
        panic!("expected default save effect");
    };

    assert_eq!(
        payload["runtimeConfig"]["audioOutputs"],
        json!({ "dac": false, "usb": true, "hdmi": false })
    );
    assert!(payload["runtimeConfig"]["usb"]
        .as_object()
        .unwrap()
        .get("audioOut")
        .is_none());

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(
        restored.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": false, "usb": true, "hdmi": false })
    );
    assert!(restored.config_payload()["runtimeConfig"]["usb"]
        .as_object()
        .unwrap()
        .get("audioOut")
        .is_none());
}

#[test]
pub(crate) fn legacy_config_is_migrated_to_current_envelope() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut legacy = source.config_payload();
    let object = legacy.as_object_mut().unwrap();
    object.remove("kind");
    object.remove("schemaVersion");
    object.remove("revision");
    object
        .get_mut("runtimeConfig")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("audioOutputs");

    source.apply_config_payload(legacy).unwrap();
    let migrated = source.config_payload();

    assert_eq!(migrated["kind"], "octessera.config");
    assert_eq!(migrated["schemaVersion"], 2);
    assert!(migrated["runtimeConfig"].is_object());
    assert_eq!(
        migrated["runtimeConfig"]["audioOutputs"],
        json!({ "dac": true, "usb": false, "hdmi": false })
    );
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
    runner.xy_touch.active = true;
    runner.xy_touch.x = 0.23;
    runner.active_sparks_fx = vec![("mixer.volume".into(), "instruments.0".into())];
    runner.sample_assign = Some((1, 2));
    let before_payload = runner.config_payload();
    let before_snapshot = runner.snapshot().unwrap();
    let before_transport = runner.transport.clone();
    let before_audio_revision = runner.audio_config_revision;
    let before_xy_touch = runner.xy_touch.clone();
    let before_active_sparks_fx = runner.active_sparks_fx.clone();
    let before_sample_assign = runner.sample_assign;
    let mut invalid = before_payload.clone();
    invalid["runtimeConfig"]["layers"][0]["worlds"]["behaviorId"] = json!("unsupported-behavior");

    assert!(runner.apply_config_payload(invalid).is_err());

    assert_eq!(runner.config_payload(), before_payload);
    assert_eq!(runner.snapshot().unwrap(), before_snapshot);
    assert_eq!(runner.transport, before_transport);
    assert_eq!(runner.audio_config_revision, before_audio_revision);
    assert_eq!(runner.menu.state.stack, vec![0, 1]);
    assert_eq!(runner.menu.state.cursor, 2);
    assert_eq!(runner.xy_touch, before_xy_touch);
    assert_eq!(runner.active_sparks_fx, before_active_sparks_fx);
    assert_eq!(runner.sample_assign, before_sample_assign);
}

#[test]
pub(crate) fn failed_config_preparation_retains_pending_persistence_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.mark_fast_autosave_dirty();
    let before_payload = runner.config_payload();
    let before_dirty_revision = runner.dirty_revision;
    let before_pending_autosave = runner.pending.pending_autosave_payload_due_at;
    let mut invalid = before_payload.clone();
    invalid["runtimeConfig"]["masterVolume"] = json!(u64::MAX);

    assert!(runner.apply_config_payload(invalid).is_err());

    assert_eq!(runner.config_payload(), before_payload);
    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, before_dirty_revision);
    assert_eq!(
        runner.pending.pending_autosave_payload_due_at,
        before_pending_autosave
    );
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
