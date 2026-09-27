use super::music_first_tests::{assert_music_only, playing_default};
use super::*;
use crate::tests::support::FakeHost;
use std::sync::Arc;

#[test]
fn native_save_result_is_music_first_without_snapshot_or_payload_serialization() {
    let mut runner = playing_default();
    let initial = runner.capture_display_scene().unwrap();
    runner.acknowledge_display_scene(initial.generation());
    assert!(!runner.display_scene_pending());
    runner.mark_config_dirty();
    let revision = runner.config_revision;
    let serialization_calls = runner.behavior_state_serialization_calls.get();
    let worker_payload = Arc::new(json!({
        "revision": revision,
        "runtimeConfig": { "usb": { "dataRole": "device" } }
    }));
    assert!(runner.register_native_default_write("native-music-save", revision, true));
    assert!(runner.attach_native_default_write_payload(
        "native-music-save",
        revision,
        Arc::clone(&worker_payload),
    ));
    let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig::default());
    let mut host = FakeHost::default();

    let output = runtime
        .dispatch_host_message_music_first(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::Identified {
                    result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                        ok: true,
                        is_auto: Some(true),
                    }),
                    request_id: "native-music-save".into(),
                    revision: Some(revision),
                },
            },
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(output
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::RuntimeStatus { .. })));
    assert!(!output
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert_eq!(
        runtime.last_status().unwrap().transport,
        RuntimeTransportState::Playing
    );
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Saved default")
    );
    assert!(Arc::ptr_eq(
        &runner.restart_settings.persisted_default,
        &worker_payload
    ));
    assert!(!runner.config_dirty);
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );
    assert!(runner.display_scene_pending());
    let saved_scene = runner.capture_display_scene().unwrap();
    let saved_generation = saved_scene.generation();
    let scene_snapshot = saved_scene.into_snapshot();
    assert_eq!(scene_snapshot["display"]["toast"], "Saved default");
    assert_eq!(scene_snapshot["settings"]["autoSaveFlash"], "flash");
    runner.acknowledge_display_scene(saved_generation);
    assert!(!runner.display_scene_pending());
}

#[test]
fn successful_native_backup_is_music_first_without_replacing_default_baseline() {
    let mut runner = playing_default();
    let initial = runner.capture_display_scene().unwrap();
    runner.acknowledge_display_scene(initial.generation());
    let baseline = Arc::clone(&runner.restart_settings.persisted_default);
    let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig::default());
    let mut host = FakeHost::default();
    let output = runtime
        .dispatch_host_message_music_first(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::Identified {
                    result: Box::new(RuntimeStoreResult::SaveBackupResult { ok: true }),
                    request_id: "native-backup".into(),
                    revision: Some(runner.config_revision),
                },
            },
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(output
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::RuntimeStatus { .. })));
    assert!(!output
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert!(Arc::ptr_eq(
        &runner.restart_settings.persisted_default,
        &baseline
    ));
    assert!(!runner.display_scene_pending());
}

#[test]
fn stale_unidentified_and_mismatched_native_default_successes_remain_synchronous() {
    for result_kind in ["unidentified", "wrong-id", "wrong-revision"] {
        let mut runner = playing_default();
        runner.mark_config_dirty();
        let revision = runner.config_revision;
        let baseline = Arc::clone(&runner.restart_settings.persisted_default);
        let worker_payload = Arc::new(json!({ "revision": revision }));
        assert!(runner.register_native_default_write("native-current", revision, true));
        assert!(runner.attach_native_default_write_payload(
            "native-current",
            revision,
            Arc::clone(&worker_payload),
        ));
        let current_revision = revision;
        let flash_serial = runner.display.auto_save_flash_serial;
        let result = match result_kind {
            "unidentified" => RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            },
            "wrong-id" => RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                    ok: true,
                    is_auto: Some(true),
                }),
                request_id: "different-request".into(),
                revision: Some(revision),
            },
            "wrong-revision" => RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                    ok: true,
                    is_auto: Some(true),
                }),
                request_id: "native-current".into(),
                revision: Some(revision + 1),
            },
            _ => unreachable!(),
        };
        let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig::default());
        let mut host = FakeHost::default();

        let output = runtime
            .dispatch_host_message_music_first(
                HostMessage::RuntimeResult { result },
                &mut runner,
                &mut host,
            )
            .unwrap();

        assert!(
            output
                .messages
                .iter()
                .any(|message| matches!(message, RunnerMessage::Snapshot { .. })),
            "result kind: {result_kind}"
        );
        assert!(!Arc::ptr_eq(
            &runner.restart_settings.persisted_default,
            &worker_payload
        ));
        assert!(Arc::ptr_eq(
            &runner.restart_settings.persisted_default,
            &baseline
        ));
        assert!(runner.config_dirty);
        assert_eq!(runner.dirty_revision, Some(current_revision));
        assert_eq!(
            runner.restart_settings.pending_write_revision(),
            Some(revision)
        );
        assert_eq!(runner.display.auto_save_flash_serial, flash_serial);
    }
}

#[test]
fn missing_native_payload_success_forces_synchronous_error_presentation() {
    let mut runner = playing_default();
    runner.auto_save_default = true;
    runner.mark_config_dirty();
    let revision = runner.config_revision;
    assert!(runner.register_native_default_write("native-missing-result", revision, true));
    let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig::default());
    let mut host = FakeHost::default();
    let output = runtime
        .dispatch_host_message_music_first(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::Identified {
                    result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                        ok: true,
                        is_auto: Some(true),
                    }),
                    request_id: "native-missing-result".into(),
                    revision: Some(revision),
                },
            },
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(output.messages.iter().any(|message| matches!(message,
        RunnerMessage::Snapshot { snapshot }
            if snapshot["display"]["title"] == "RUNTIME ERROR"
    )));
    assert!(runner.display.runtime_error_presentation.is_some());
    assert!(runner.config_dirty);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
}

#[test]
fn failed_default_result_remains_on_the_synchronous_presentation_path() {
    let mut runner = playing_default();
    let messages = runner
        .send_music_first(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                    ok: false,
                    is_auto: Some(true),
                }),
                request_id: "failed-default".into(),
                revision: Some(runner.config_revision),
            },
        })
        .unwrap();

    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
fn due_deferred_autosave_stays_dirty_until_separate_persistence_call() {
    let mut runner = playing_default();
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.synth.osc1.levelPct".into()),
        press_action: None,
    });
    runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "encoder_turn", "id": "aux1", "delta": 1}),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    runner.make_deferred_menu_apply_due_for_test();
    let due = runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap();
    assert_music_only(&due);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    assert!(runner.display_scene_pending());
}

#[test]
fn due_music_first_save_follows_notes_without_snapshot_and_reloads_edited_value() {
    let mut runner = playing_default();
    runner.auto_save_default = true;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.synth.osc1.levelPct".into()),
        press_action: None,
    });
    runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "encoder_turn", "id": "aux1", "delta": 1}),
            request_snapshot: Some(false),
        })
        .unwrap();
    runner.make_deferred_menu_apply_due_for_test();
    runner.audio_config_revision += 1;
    let musical = runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_music_only(&musical);
    let config = musical.iter().position(|message| matches!(message, RunnerMessage::AudioCommands { commands }
        if commands.iter().any(|command| matches!(command, RuntimeAudioCommand::SetAudioConfig { .. })))).unwrap();
    let note = musical
        .iter()
        .position(|message| matches!(message, RunnerMessage::MusicalEvents { .. }))
        .unwrap();
    assert!(config < note);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    runner.last_backup_save_at = None;
    let save = runner.flush_due_persistence_music_first().unwrap();
    assert!(!save
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    let payload = save
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => {
                effects.iter().find_map(|effect| match effect {
                    RuntimePlatformEffect::StoreSaveDefault { payload, mode }
                        if mode.as_deref() == Some("deferred") =>
                    {
                        Some(payload.clone())
                    }
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("deferred save after host musical delivery");
    let backup = save
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => {
                effects.iter().find_map(|effect| match effect {
                    RuntimePlatformEffect::StoreSaveBackup { payload } => Some(payload),
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("due rolling backup");
    assert_eq!(backup, &payload);
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["levelPct"],
        81
    );
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.instruments[0].synth_config["osc1"]["levelPct"], 81);
}

#[test]
fn host_receives_due_audio_and_notes_before_explicit_save_serialization() {
    let mut runner = playing_default();
    runner.auto_save_default = true;
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.synth.osc1.levelPct".into()),
        press_action: None,
    });
    let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig::default());
    let mut host = FakeHost::default();
    let initial = runner.messages_with_snapshot().unwrap();
    runtime
        .dispatch_runner_messages(initial, &mut runner, &mut host)
        .unwrap();
    runtime
        .dispatch_host_message_music_first(
            HostMessage::DeviceInput {
                input: json!({"type": "encoder_turn", "id": "aux1", "delta": 1}),
                request_snapshot: Some(false),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    runner.make_deferred_menu_apply_due_for_test();
    runner.audio_config_revision += 1;
    let before_notes = host.musical_events.len();
    let before_audio = host.audio_commands.len();
    let output = runtime
        .dispatch_host_message_music_first(
            HostMessage::TransportPulseStep {
                pulses: 24,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_music_only(&output.messages);
    assert!(host.musical_events.len() > before_notes);
    assert!(host.audio_commands[before_audio..]
        .iter()
        .any(|command| matches!(command, RuntimeAudioCommand::SetAudioConfig { .. })));
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    let saved = runner.flush_due_persistence_music_first().unwrap();
    assert!(saved.iter().any(|message| matches!(message, RunnerMessage::PlatformEffects { effects }
        if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. })))));
}

#[test]
fn failed_music_first_default_write_remains_dirty_and_reschedules_with_visible_error() {
    let mut runner = playing_default();
    runner.auto_save_default = true;
    runner.rolling_backups = false;
    runner.mark_fast_autosave_dirty();
    runner.make_deferred_menu_apply_due_for_test();
    runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap();
    let save = runner.flush_due_persistence_music_first().unwrap();
    assert!(save.iter().any(|message| matches!(message, RunnerMessage::PlatformEffects { effects }
        if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. })))));
    let revision = runner.config_revision;
    runner.register_default_write_request("failed-save", Some(revision));
    let failed = RuntimeStoreResult::RuntimeFailure {
        error: crate::RuntimeErrorFacts::new(
            crate::RuntimeErrorDomain::Storage,
            crate::RuntimeErrorCode::OperationFailed,
            crate::RuntimeOperation::StoreSaveDefault,
            Some("disk unavailable".into()),
        ),
    }
    .with_identity("failed-save".into(), Some(revision));
    let response = runner
        .send(HostMessage::RuntimeResult { result: failed })
        .unwrap();
    assert!(response.iter().any(
        |message| matches!(message, RunnerMessage::Snapshot { snapshot }
        if snapshot["display"]["title"] == "RUNTIME ERROR")
    ));
    assert!(runner.config_dirty);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    let retry_due = runner.pending.pending_autosave_payload_due_at.unwrap();
    assert!(runner
        .persistence_intent_at(retry_due - std::time::Duration::from_millis(1))
        .is_none());
    assert_eq!(
        runner.persistence_intent_at(retry_due).unwrap().revision(),
        revision
    );
    runner.make_deferred_menu_apply_due_for_test();
    let retry = runner.flush_due_persistence_music_first().unwrap();
    assert!(retry.iter().any(|message| matches!(message, RunnerMessage::PlatformEffects { effects }
        if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. })))));
}

#[test]
fn shipped_backup_only_defaults_do_not_block_future_backups_with_stale_save_due() {
    let mut runner = playing_default();
    assert!(!runner.auto_save_default);
    runner.mark_fast_autosave_dirty();
    runner.make_deferred_menu_apply_due_for_test();
    runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap();
    runner.last_backup_save_at = None;
    let effects = runner.flush_due_persistence_music_first().unwrap();
    assert!(effects.iter().any(|message| matches!(message, RunnerMessage::PlatformEffects { effects }
        if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveBackup { .. })))));
    assert!(!effects.iter().any(|message| matches!(message, RunnerMessage::PlatformEffects { effects }
        if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. })))));
    assert!(runner.config_dirty);
    assert!(runner.pending.pending_autosave_payload_due_at.is_none());
}
