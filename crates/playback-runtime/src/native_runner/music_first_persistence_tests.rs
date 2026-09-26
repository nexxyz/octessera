use super::music_first_tests::{assert_music_only, playing_default};
use super::*;
use crate::tests::support::FakeHost;

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
