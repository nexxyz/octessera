use super::restart_settings::DefaultSaveScope;
use super::*;
use std::sync::Arc;
use std::time::Duration;

fn payload(revision: u64) -> Arc<Value> {
    Arc::new(json!({
        "revision": revision,
        "runtimeConfig": { "usb": { "dataRole": "device" } }
    }))
}

fn send_save_result(
    runner: &mut NativeRunner,
    request_id: &str,
    revision: u64,
    ok: bool,
) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result: Box::new(if ok {
                    RuntimeStoreResult::SaveDefaultResult {
                        ok: true,
                        is_auto: Some(true),
                    }
                } else {
                    RuntimeStoreResult::RuntimeFailure {
                        error: crate::RuntimeErrorFacts::new(
                            crate::RuntimeErrorDomain::Storage,
                            crate::RuntimeErrorCode::OperationFailed,
                            crate::RuntimeOperation::StoreSaveDefault,
                            Some("native save failed".into()),
                        ),
                    }
                }),
                request_id: request_id.into(),
                revision: Some(revision),
            },
        })
        .unwrap()
}

#[test]
fn native_registration_and_payload_attachment_are_revision_checked_and_zero_clone() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.mark_config_dirty();
    let revision = runner.config_revision;
    let serialization_calls = runner.behavior_state_serialization_calls.get();
    let baseline = Arc::clone(&runner.restart_settings.persisted_default);

    assert!(runner.register_native_default_write("native-save-1", revision, true));
    assert_eq!(
        runner.restart_settings.pending_write_revision(),
        Some(revision)
    );
    assert_eq!(runner.pending.pending_save_revision, Some(revision));
    assert!(Arc::ptr_eq(
        &runner.restart_settings.persisted_default,
        &baseline
    ));
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );

    assert!(!runner.attach_native_default_write_payload("wrong-id", revision, payload(revision),));
    assert!(!runner.attach_native_default_write_payload(
        "native-save-1",
        revision + 1,
        payload(revision + 1),
    ));
    assert!(!runner.attach_native_default_write_payload(
        "native-save-1",
        revision,
        payload(revision + 1),
    ));
    let worker_payload = payload(revision);
    assert!(runner.attach_native_default_write_payload(
        "native-save-1",
        revision,
        Arc::clone(&worker_payload),
    ));
}

#[test]
fn native_success_installs_the_same_worker_arc_and_clears_only_its_revision() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.mark_config_dirty();
    let revision = runner.config_revision;
    assert!(runner.register_native_default_write("native-save-2", revision, true));
    let worker_payload = payload(revision);
    assert!(runner.attach_native_default_write_payload(
        "native-save-2",
        revision,
        Arc::clone(&worker_payload),
    ));

    send_save_result(&mut runner, "native-save-2", revision, true);

    assert!(Arc::ptr_eq(
        &runner.restart_settings.persisted_default,
        &worker_payload
    ));
    assert!(runner.restart_settings.pending_write_revision().is_none());
    assert!(runner.pending.pending_save_revision.is_none());
    assert!(!runner.config_dirty);
    assert_eq!(runner.dirty_revision, None);
}

#[test]
fn native_manual_write_uses_the_existing_ordinary_save_scope() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.mark_config_dirty();
    let revision = runner.config_revision;
    assert!(runner.register_native_default_write("native-manual-save", revision, false));
    let worker_payload = payload(revision);
    assert!(runner.attach_native_default_write_payload(
        "native-manual-save",
        revision,
        Arc::clone(&worker_payload),
    ));

    send_save_result(&mut runner, "native-manual-save", revision, true);

    assert!(Arc::ptr_eq(
        &runner.restart_settings.persisted_default,
        &worker_payload
    ));
    assert!(!runner.config_dirty);
}

#[test]
fn old_native_success_keeps_newer_edit_dirty() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.mark_config_dirty();
    let old_revision = runner.config_revision;
    assert!(runner.register_native_default_write("native-save-old", old_revision, true));
    let old_payload = payload(old_revision);
    assert!(runner.attach_native_default_write_payload(
        "native-save-old",
        old_revision,
        Arc::clone(&old_payload),
    ));
    runner.mark_config_dirty();
    let new_revision = runner.config_revision;

    send_save_result(&mut runner, "native-save-old", old_revision, true);

    assert!(Arc::ptr_eq(
        &runner.restart_settings.persisted_default,
        &old_payload
    ));
    assert_eq!(runner.pending.pending_save_revision, None);
    assert!(runner.config_dirty);
    assert_eq!(runner.dirty_revision, Some(new_revision));
}

#[test]
fn native_failure_or_missing_payload_keeps_baseline_and_schedules_retry() {
    for attach_payload in [false, true] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.auto_save_default = true;
        runner.mark_config_dirty();
        let revision = runner.config_revision;
        let baseline = Arc::clone(&runner.restart_settings.persisted_default);
        assert!(runner.register_native_default_write("native-save-fail", revision, true));
        if attach_payload {
            assert!(runner.attach_native_default_write_payload(
                "native-save-fail",
                revision,
                payload(revision),
            ));
        }

        let _messages = send_save_result(&mut runner, "native-save-fail", revision, false);

        assert!(Arc::ptr_eq(
            &runner.restart_settings.persisted_default,
            &baseline
        ));
        assert!(runner.config_dirty);
        assert!(runner.pending.pending_autosave_payload_due_at.is_some());
        assert!(runner.restart_settings.pending_write_revision().is_none());
        assert!(runner.display.runtime_error_presentation.is_some());
    }

    let mut missing = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    missing.auto_save_default = true;
    missing.display.toast = None;
    missing.display.toast_expires_at = None;
    missing.display.modifier_hint_started_at = None;
    missing.mark_config_dirty();
    let revision = missing.config_revision;
    let baseline = Arc::clone(&missing.restart_settings.persisted_default);
    assert!(missing.register_native_default_write("native-save-missing", revision, true));

    let _messages = send_save_result(&mut missing, "native-save-missing", revision, true);

    assert!(Arc::ptr_eq(
        &missing.restart_settings.persisted_default,
        &baseline
    ));
    assert!(missing.config_dirty);
    assert!(missing.pending.pending_autosave_payload_due_at.is_some());
    assert!(missing.restart_settings.pending_write_revision().is_none());
    assert!(missing.display.runtime_error_presentation.is_some());
}

#[test]
fn attached_native_host_role_is_used_for_existing_restart_follow_up() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.mark_config_dirty();
    let revision = runner.config_revision;
    assert!(runner.restart_settings.register_native_write(
        "native-host-save",
        revision,
        DefaultSaveScope::RestartEverything,
    ));
    runner.pending.pending_save_revision = Some(revision);
    let host_payload = Arc::new(json!({
        "revision": revision,
        "runtimeConfig": { "usb": { "dataRole": "host" } }
    }));
    assert!(runner.attach_native_default_write_payload(
        "native-host-save",
        revision,
        Arc::clone(&host_payload),
    ));

    send_save_result(&mut runner, "native-host-save", revision, true);

    assert!(Arc::ptr_eq(
        &runner.restart_settings.persisted_default,
        &host_payload
    ));
    assert!(
        runner
            .display
            .confirm_dialog
            .as_ref()
            .unwrap()
            .lines
            .iter()
            .any(|line| line == "Unplug computer USB"),
        "{:?}",
        runner.display.confirm_dialog
    );
}

#[test]
fn retry_deadline_remains_the_existing_150ms_after_native_failure() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.mark_config_dirty();
    let revision = runner.config_revision;
    assert!(runner.register_native_default_write("native-retry", revision, true));
    send_save_result(&mut runner, "native-retry", revision, false);
    let due = runner.pending.pending_autosave_payload_due_at.unwrap();
    assert!(due > Instant::now());
    assert!(due.duration_since(Instant::now()) <= Duration::from_millis(150));
}
