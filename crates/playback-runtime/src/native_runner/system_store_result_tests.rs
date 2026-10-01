use super::restart_settings::DefaultSaveScope;
use super::*;
use crate::{CoreRunner, RuntimeOperation, RuntimePlatformRequest};

fn register_system_request(
    runner: &mut NativeRunner,
    operation: RuntimeOperation,
    request_id: &str,
    revision: Option<u64>,
) -> RuntimePlatformRequest {
    let effect = match operation {
        RuntimeOperation::StoreLoadSystem => RuntimePlatformEffect::StoreLoadSystem,
        RuntimeOperation::StoreSaveSystem => RuntimePlatformEffect::StoreSaveSystem {
            payload: system_document(runner),
        },
        _ => unreachable!(),
    };
    let request = RuntimePlatformRequest::new(effect, request_id.into(), revision);
    runner.register_platform_request(&request);
    request
}

fn system_document(runner: &NativeRunner) -> Value {
    split_system_patch_documents(&runner.config_payload())
        .unwrap()
        .system
}

#[test]
fn system_store_load_result_updates_device_display_without_disturbing_patch_runtime() {
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
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.mark_config_dirty();
    runner.pending.pending_save_revision = Some(42);
    let patch = runner.patch_payload().unwrap();
    let revision_state = (
        runner.config_revision,
        runner.dirty_revision,
        runner.config_dirty,
        runner.pending.pending_save_revision,
        runner.pending.pending_autosave_payload_due_at,
    );
    let baseline = runner.restart_settings.clone();
    let mut system = system_document(&runner);
    system["runtimeConfig"]["displayBrightness"] = json!(27);
    system["runtimeConfig"]["ghostCells"] = json!(true);
    register_system_request(
        &mut runner,
        RuntimeOperation::StoreLoadSystem,
        "system-load-1",
        Some(42),
    );

    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult {
                payload: Some(system.clone()),
            }
            .with_identity("system-load-1".into(), Some(42)),
        })
        .unwrap();

    assert_eq!(runner.display.ui.display_brightness, 27);
    assert!(runner.display.ui.ghost_cells);
    assert_eq!(runner.menu.number_for_key("displayBrightness"), Some(27));
    assert_eq!(
        runner.menu.value_for_key("ghostCells").as_deref(),
        Some("true")
    );
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.engine.drain_held_notes(usize::MAX).len(), 1);
    assert_eq!(runner.patch_payload().unwrap(), patch);
    assert_eq!(
        (
            runner.config_revision,
            runner.dirty_revision,
            runner.config_dirty,
            runner.pending.pending_save_revision,
            runner.pending.pending_autosave_payload_due_at,
        ),
        revision_state
    );
    assert_eq!(&*runner.restart_settings.persisted_default, &system);
    assert_eq!(
        runner.restart_settings.pending_write_revision(),
        baseline.pending_write_revision()
    );
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "System loaded"
    );
}

#[test]
fn system_store_load_missing_or_invalid_result_is_a_typed_non_mutating_storage_error() {
    for payload in [None, Some(json!({ "invalid": true }))] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.mark_config_dirty();
        runner.pending.pending_save_revision = Some(17);
        let before = runner.config_payload();
        let revision_state = (
            runner.config_revision,
            runner.dirty_revision,
            runner.config_dirty,
            runner.pending.pending_save_revision,
        );
        register_system_request(
            &mut runner,
            RuntimeOperation::StoreLoadSystem,
            "identified-system-load",
            Some(17),
        );
        runner
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadSystemResult { payload }
                    .with_identity("identified-system-load".into(), Some(17)),
            })
            .unwrap();

        assert_eq!(runner.config_payload(), before);
        assert_eq!(
            (
                runner.config_revision,
                runner.dirty_revision,
                runner.config_dirty,
                runner.pending.pending_save_revision,
            ),
            revision_state
        );
        let error = runner.display.runtime_error_presentation.as_ref().unwrap();
        assert!(error
            .lines
            .iter()
            .any(|line| line.starts_with("OP store load")));
        assert!(error.lines.iter().any(|line| line.contains("storage")));
    }
}

#[test]
fn system_store_save_results_never_acknowledge_patch_dirty_state() {
    for ok in [true, false] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.mark_config_dirty();
        let patch_revision = runner.config_revision;
        runner.pending.pending_save_revision = Some(patch_revision);
        let patch = super::portable_patch_payload_for_save(&runner.config_payload()).unwrap();
        assert!(runner
            .register_default_write(patch, super::restart_settings::DefaultSaveScope::Ordinary,));
        runner.register_default_write_request("default-restart", Some(patch_revision));
        let restart_settings = runner.restart_settings.clone();
        let system_baseline = system_document(&runner);
        let dirty_revision = runner.dirty_revision;
        register_system_request(
            &mut runner,
            RuntimeOperation::StoreSaveSystem,
            "identified-system-save",
            Some(29),
        );
        runner
            .apply_store_result(
                RuntimeStoreResult::SaveSystemResult { ok }
                    .with_identity("identified-system-save".into(), Some(29)),
            )
            .unwrap();

        assert!(runner.config_dirty);
        assert_eq!(runner.dirty_revision, dirty_revision);
        assert_eq!(runner.pending.pending_save_revision, Some(patch_revision));
        assert_eq!(
            runner.pending_default_write_revision(),
            Some(patch_revision)
        );
        if ok {
            assert_eq!(
                &*runner.restart_settings.persisted_default,
                &system_baseline
            );
            assert_eq!(
                runner.restart_settings.pending_write_revision(),
                restart_settings.pending_write_revision()
            );
            assert_eq!(
                runner.display.toast.as_ref().unwrap().message,
                "System saved"
            );
        } else {
            assert_eq!(runner.restart_settings, restart_settings);
            let error = runner.display.runtime_error_presentation.as_ref().unwrap();
            assert!(error
                .lines
                .iter()
                .any(|line| line.starts_with("OP store save")));
            assert!(error.lines.iter().any(|line| line.contains("storage")));
        }
    }
}

#[test]
fn apply_system_save_host_role_uses_system_payload_for_restart_follow_up() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.mark_system_dirty();
    let mut system =
        super::system_persistence::SystemPersistenceState::system_document(&runner).unwrap();
    system["runtimeConfig"]["usb"]["dataRole"] = json!("host");
    system["runtimeConfig"]["audioOutputs"]["usb"] = json!(false);
    system["runtimeConfig"]["usb"]["midiOutEnabled"] = json!(false);
    runner.start_restart_system_save(system, DefaultSaveScope::RestartSetting);
    let messages = runner.messages_with_snapshot().unwrap();
    let effect = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                matches!(effect, RuntimePlatformEffect::StoreSaveSystem { .. })
                    .then(|| effect.clone())
            }),
            _ => None,
        })
        .expect("System Apply store effect");
    let request = RuntimePlatformRequest::new(effect, "system-apply-host".into(), None);
    runner.register_platform_request(&request);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(request.request_id, request.revision),
        })
        .unwrap();

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
