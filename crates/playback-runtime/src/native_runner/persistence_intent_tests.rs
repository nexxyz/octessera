use super::music_first_tests::{assert_music_only, playing_default};
use super::restart_settings::RestartSetting;
use super::*;
use std::time::{Duration, Instant};

fn dirty_at(runner: &mut NativeRunner, due_at: Instant) -> u64 {
    runner.mark_config_dirty();
    runner.pending.pending_autosave_payload_due_at = Some(due_at);
    runner.dirty_revision.unwrap()
}

fn sleep_until(target: Instant) {
    let now = Instant::now();
    if target > now {
        std::thread::sleep(target - now);
    }
}

#[test]
fn intent_tracks_debounce_revision_and_never_serializes_state() {
    fn assert_send<T: Send + 'static>() {}
    assert_send::<NativePersistenceIntent>();

    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.rolling_backups = false;
    runner.mark_fast_autosave_dirty();
    let due_at = runner.pending.pending_autosave_payload_due_at.unwrap();
    let revision = runner.dirty_revision.unwrap();
    let serialization_calls = runner.behavior_state_serialization_calls.get();
    let presentation_generation = runner.display.transients.generation();

    assert!(runner
        .persistence_intent_at(due_at - Duration::from_millis(1))
        .is_none());
    for _ in 0..60 {
        let intent = runner.persistence_intent_at(due_at).unwrap();
        assert_eq!(intent.revision(), revision);
        assert!(intent.default_eligible());
        assert!(!intent.backup_eligible());
    }
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );
    assert_eq!(
        runner.display.transients.generation(),
        presentation_generation
    );
}

#[test]
fn stopped_music_first_dispatch_defers_automatic_payload_but_keeps_intent_due() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.rolling_backups = false;
    let due_at = Instant::now();
    dirty_at(&mut runner, due_at);

    let messages = runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect,
                crate::RuntimePlatformEffect::StoreSaveDefault { .. }
                    | crate::RuntimePlatformEffect::StoreSaveBackup { .. }))
    )));
    assert!(runner.persistence_intent_at(due_at).is_some());

    let generic = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(generic.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect,
                crate::RuntimePlatformEffect::StoreSaveDefault { .. }))
    )));
}

#[test]
fn sixty_user_aux_edits_refresh_only_the_latest_native_persistence_intent() {
    let mut runner = playing_default();
    runner.auto_save_default = true;
    runner.rolling_backups = false;
    runner.menu.rebuild(runner.menu_config());
    assert!(runner
        .menu
        .focus_item_key("aux:0:turn.instruments.0.synth.osc1.levelPct"));
    runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("instruments.0.synth.osc1.levelPct")
    );

    let serialization_calls = runner.behavior_state_serialization_calls.get();
    let started = Instant::now() + Duration::from_millis(10);
    let mut latest_revision = runner.dirty_revision.unwrap_or(runner.config_revision);
    let mut previous_eligible_revision = None;

    for turn in 0..60 {
        let turn_at = started + Duration::from_millis(turn * 200);
        sleep_until(turn_at);
        let old_revision = runner.config_revision;
        let old_value = runner.instruments[0].synth_config["osc1"]["levelPct"]
            .as_f64()
            .unwrap();
        let delta = if turn % 2 == 0 { 1 } else { -1 };
        let messages = runner
            .send_music_first(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "id": "aux1", "delta": delta }),
                request_snapshot: Some(false),
            })
            .unwrap();

        assert_music_only(&messages);
        assert!(!messages
            .iter()
            .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
        assert_eq!(runner.config_revision, old_revision + 1);
        latest_revision = runner.dirty_revision.unwrap();
        assert_eq!(latest_revision, runner.config_revision);
        assert_ne!(latest_revision, old_revision);
        if let Some(previous) = previous_eligible_revision {
            assert_ne!(latest_revision, previous);
        }
        assert_eq!(
            runner.instruments[0].synth_config["osc1"]["levelPct"].as_f64(),
            Some(old_value + f64::from(delta))
        );
        let due_at = runner.pending.pending_autosave_payload_due_at.unwrap();
        assert!(runner.persistence_intent_at(Instant::now()).is_none());

        let eligibility_poll = due_at + Duration::from_millis(1);
        sleep_until(eligibility_poll);
        let intent = runner.persistence_intent_at(Instant::now()).unwrap();
        assert_eq!(intent.revision(), latest_revision);
        assert!(intent.default_eligible());
        assert!(!intent.backup_eligible());
        previous_eligible_revision = Some(intent.revision());

        let first_notification = runner
            .flush_deferred_menu_apply_music_first(Instant::now())
            .unwrap();
        assert!(first_notification
            .iter()
            .any(|message| matches!(message, RunnerMessage::RuntimeStatus { .. })));
        assert!(!first_notification
            .iter()
            .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
        let generation = runner.display.transients.generation();
        let repeated_notification = runner
            .flush_deferred_menu_apply_music_first(Instant::now() + Duration::from_millis(8))
            .unwrap();
        assert!(repeated_notification.is_empty());
        assert_eq!(runner.display.transients.generation(), generation);
    }

    let latest = runner.persistence_intent_at(Instant::now()).unwrap();
    assert_eq!(latest.revision(), latest_revision);
    assert!(latest.default_eligible());
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );
}

#[test]
fn intent_reports_backup_and_combined_eligibility_without_changing_legacy_rules() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = false;
    runner.rolling_backups = true;
    let start = Instant::now();
    let revision = dirty_at(&mut runner, start);
    runner.last_backup_save_at = Some(start);
    let backup = runner
        .persistence_intent_at(start + Duration::from_secs(300))
        .unwrap();
    assert_eq!(backup.revision(), revision);
    assert!(!backup.default_eligible());
    assert!(backup.backup_eligible());

    runner.auto_save_default = true;
    let combined = runner
        .persistence_intent_at(start + Duration::from_secs(300))
        .unwrap();
    assert!(combined.default_eligible());
    assert!(combined.backup_eligible());
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
}

#[test]
fn native_backup_issuance_advances_only_backup_eligibility() {
    let now = Instant::now();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.rolling_backups = true;
    let revision = dirty_at(&mut runner, now);
    let before = runner.persistence_intent_at(now).unwrap();
    assert!(before.default_eligible());
    assert!(before.backup_eligible());

    runner.mark_native_backup_issued_at(now);
    let after = runner.persistence_intent_at(now).unwrap();
    assert_eq!(after.revision(), revision);
    assert!(after.default_eligible());
    assert!(!after.backup_eligible());
    assert!(runner.config_dirty);

    let backup_due = runner
        .persistence_intent_at(now + Duration::from_secs(300))
        .unwrap();
    assert!(backup_due.default_eligible());
    assert!(backup_due.backup_eligible());
}

#[test]
fn music_first_due_notification_is_one_shot_while_intent_stays_eligible() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.rolling_backups = false;
    let due_at = Instant::now() + Duration::from_millis(150);
    let revision = dirty_at(&mut runner, due_at);
    let generation = runner.display.transients.generation();

    let first = runner
        .flush_deferred_menu_apply_music_first(due_at)
        .unwrap();
    assert!(!first.is_empty());
    assert_eq!(runner.display.transients.generation(), generation + 1);
    assert_eq!(runner.pending.pending_autosave_payload_due_at, Some(due_at));

    let repeated = runner
        .flush_deferred_menu_apply_music_first(due_at + Duration::from_millis(8))
        .unwrap();
    assert!(repeated.is_empty());
    assert_eq!(runner.display.transients.generation(), generation + 1);
    assert_eq!(
        runner
            .persistence_intent_at(due_at + Duration::from_millis(8))
            .unwrap()
            .revision(),
        revision
    );
}

#[test]
fn restart_and_restore_gates_match_existing_save_eligibility() {
    let due_at = Instant::now();
    let mut editing = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    editing.auto_save_default = true;
    editing.rolling_backups = false;
    dirty_at(&mut editing, due_at);
    editing.restart_settings.begin_edit(
        &json!({ "runtimeConfig": { "audioOutputs": { "dac": true } } }),
        RestartSetting::AudioOutputDac,
    );
    assert!(editing.persistence_intent_at(due_at).is_none());
    editing.rolling_backups = true;
    editing.last_backup_save_at = Some(due_at - Duration::from_secs(300));
    let backup_while_editing = editing.persistence_intent_at(due_at).unwrap();
    assert!(!backup_while_editing.default_eligible());
    assert!(backup_while_editing.backup_eligible());

    let mut pending_write = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    pending_write.auto_save_default = true;
    pending_write.rolling_backups = false;
    let revision = dirty_at(&mut pending_write, due_at);
    pending_write.pending.pending_save_revision = Some(revision);
    assert!(pending_write.persistence_intent_at(due_at).is_none());
    pending_write.rolling_backups = true;
    pending_write.last_backup_save_at = Some(due_at - Duration::from_secs(300));
    let backup_while_pending = pending_write.persistence_intent_at(due_at).unwrap();
    assert!(!backup_while_pending.default_eligible());
    assert!(backup_while_pending.backup_eligible());

    let mut restoring = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restoring.auto_save_default = true;
    restoring.rolling_backups = true;
    dirty_at(&mut restoring, due_at);
    restoring.apply_user_data_restore_status(
        RuntimeUserDataRestoreStatus {
            phase: RuntimeUserDataRestorePhase::Restoring,
        },
        Some("restore".into()),
        Some(1),
    );
    assert!(restoring.persistence_intent_at(due_at).is_none());
}
