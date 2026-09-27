use super::super::music_first_tests::playing_default;
use super::super::*;
use crate::tests::support::FakeHost;
use crate::{NativeStoreRequest, PlaybackRuntime, RuntimeIngest, RuntimeOperation};

fn dispatch_input(
    runner: &mut NativeRunner,
    runtime: &mut PlaybackRuntime,
    host: &mut FakeHost,
    input: Value,
) -> RuntimeIngest {
    runtime
        .dispatch_host_message_music_first(
            HostMessage::DeviceInput {
                input,
                request_snapshot: Some(false),
            },
            runner,
            host,
        )
        .unwrap()
}

fn press(
    runner: &mut NativeRunner,
    runtime: &mut PlaybackRuntime,
    host: &mut FakeHost,
) -> RuntimeIngest {
    dispatch_input(
        runner,
        runtime,
        host,
        json!({ "type": "encoder_press", "id": "main" }),
    )
}

fn turn(
    runner: &mut NativeRunner,
    runtime: &mut PlaybackRuntime,
    host: &mut FakeHost,
) -> RuntimeIngest {
    dispatch_input(
        runner,
        runtime,
        host,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    )
}

fn prepare_native_rename() -> (NativeRunner, PlaybackRuntime, FakeHost, NativeStoreRequest) {
    let mut runner = playing_default();
    runner.rolling_backups = false;
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    runner.preset_names = vec!["Source".into(), "Other".into()];
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("preset.renamePick.Source"));
    let _ = press(&mut runner, &mut runtime, &mut host);
    runner.preset_draft_name = "Target".into();
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("preset.rename.apply"));
    let _ = press(&mut runner, &mut runtime, &mut host);
    let opened = runner.display.confirm_dialog.as_ref().unwrap();
    assert_eq!(opened.title, "Confirm Rename");
    let _ = turn(&mut runner, &mut runtime, &mut host);
    let prepared = press(&mut runner, &mut runtime, &mut host);
    assert!(prepared
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
    let request = runner.take_manual_save_request().unwrap();
    assert_eq!(
        request,
        NativeManualSaveRequest::Preset {
            name: "Target".into(),
            mode: None,
            rename_from: Some("Source".into()),
        }
    );
    runner.mark_config_dirty();
    let native_request = runtime
        .next_native_store_request(RuntimeOperation::StoreSavePreset, runner.config_revision);
    assert!(runner.register_native_preset_write(
        native_request.request_id(),
        native_request.revision(),
        request,
    ));
    (runner, runtime, host, native_request)
}

fn save_result(request_id: &str, revision: u64, name: &str) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SavePresetResult {
                name: name.into(),
                outcome: "created".into(),
            }),
            request_id: request_id.into(),
            revision: Some(revision),
        },
    }
}

#[test]
fn matching_preset_result_uses_frozen_rename_context_and_authoritative_catalog_once() {
    let (mut runner, mut runtime, mut host, request) = prepare_native_rename();
    assert!(!runner.attach_native_preset_catalog(
        request.request_id(),
        request.revision(),
        vec!["Later".into(), "Other".into()],
        None,
    ));
    assert!(runner.attach_native_preset_catalog(
        request.request_id(),
        request.revision(),
        vec!["Later".into(), "Other".into(), "Target".into()],
        None,
    ));
    runner.preset_rename_source = Some("Later".into());
    runner.preset_draft_name = "Changed after queue".into();
    assert!(runner.menu.focus_item_key("preset.library.load"));
    assert!(matches!(
        runner.menu.press(),
        Some(crate::native_menu::NativeMenuPressResult::EnteredGroup)
    ));
    runner.menu.turn(1);
    assert_eq!(runner.menu.current_label(), Some("Other"));
    let rebuild_count = runner.menu.rebuild_count;
    let serialization_calls = runner.behavior_state_serialization_calls.get();

    let output = runtime
        .dispatch_host_message_music_first(
            save_result(request.request_id(), request.revision(), "Target"),
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
    assert_eq!(runner.current_preset_name.as_deref(), Some("Target"));
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );
    assert_eq!(runner.preset_names, vec!["Later", "Other", "Target"]);
    assert_eq!(runner.preset_rename_source.as_deref(), Some("Later"));
    assert_eq!(runner.menu.current_label(), Some("Other"));
    assert_eq!(runner.menu.rebuild_count, rebuild_count);
    for key in [
        "preset.load",
        "preset.library.load",
        "preset.library.delete",
        "preset.library.rename",
    ] {
        let group = runner.menu.item_for_key(key).unwrap();
        assert!(group.children.iter().any(|item| item.label == "Later"));
        assert!(group.children.iter().any(|item| item.label == "Target"));
    }
    assert!(host.effects.is_empty());
    assert!(runner.display_scene_pending());
    let scene = runner.capture_display_scene().unwrap();
    let scene_generation = scene.generation();
    assert_eq!(scene.into_snapshot()["display"]["toast"], "Saved Target");
    runner.acknowledge_display_scene(scene_generation);

    let duplicate = runtime
        .dispatch_host_message_music_first(
            save_result(request.request_id(), request.revision(), "Target"),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(duplicate
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert_eq!(runner.menu.rebuild_count, rebuild_count);
    assert_eq!(runner.preset_rename_source.as_deref(), Some("Later"));
    assert!(host.effects.is_empty());
}

#[test]
fn successful_saved_snapshot_does_not_report_newer_dirty_revision_as_saved() {
    let (mut runner, mut runtime, mut host, request) = prepare_native_rename();
    assert!(runner.attach_native_preset_catalog(
        request.request_id(),
        request.revision(),
        vec!["Other".into(), "Source".into(), "Target".into()],
        None,
    ));
    let saved_revision = request.revision();
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.synth.osc1.levelPct".into()),
        press_action: None,
    });
    let edit = dispatch_input(
        &mut runner,
        &mut runtime,
        &mut host,
        json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
    );
    assert!(edit
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
    assert_eq!(runner.config_revision, saved_revision + 1);
    assert_eq!(runner.dirty_revision, Some(runner.config_revision));
    let dirty_revision = runner.dirty_revision;
    let newer_toast = runner.display.toast.as_ref().unwrap().message.clone();
    let flash_serial = runner.display.auto_save_flash_serial;

    let output = runtime
        .dispatch_host_message_music_first(
            save_result(request.request_id(), saved_revision, "Target"),
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(output
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
    assert_eq!(runner.config_revision, saved_revision + 1);
    assert_eq!(runner.dirty_revision, dirty_revision);
    assert_eq!(runner.preset_names, vec!["Other", "Source", "Target"]);
    assert_eq!(runner.current_preset_name.as_deref(), Some("Target"));
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some(newer_toast.as_str())
    );
    assert_eq!(runner.display.auto_save_flash_serial, flash_serial);
    assert!(runner.display_scene_pending());
    assert!(host.effects.is_empty());
}

#[test]
fn wrong_native_preset_identity_or_target_leaves_the_pending_write_unchanged() {
    for wrong_target in [false, true] {
        let (mut runner, mut runtime, mut host, request) = prepare_native_rename();
        assert!(runner.attach_native_preset_catalog(
            request.request_id(),
            request.revision(),
            vec!["Source".into(), "Other".into(), "Target".into()],
            None,
        ));
        let baseline_names = runner.preset_names.clone();
        let baseline_dirty = runner.dirty_revision;
        let baseline_toast = runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.clone());
        let request_id = if wrong_target {
            request.request_id()
        } else {
            "native-preset-unrelated"
        };
        let target = if wrong_target { "Different" } else { "Target" };

        let output = runtime
            .dispatch_host_message_music_first(
                save_result(request_id, request.revision(), target),
                &mut runner,
                &mut host,
            )
            .unwrap();

        assert!(output
            .messages
            .iter()
            .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
        assert_eq!(runner.preset_names, baseline_names);
        assert_eq!(runner.dirty_revision, baseline_dirty);
        assert_eq!(
            runner
                .display
                .toast
                .as_ref()
                .map(|toast| toast.message.clone()),
            baseline_toast
        );
        assert_eq!(
            runner
                .pending
                .native_preset_write
                .as_ref()
                .map(|write| write.request_id.as_str()),
            Some(request.request_id())
        );
        assert!(host.effects.is_empty(), "{:?}", host.effects);
    }
}

#[test]
fn matching_native_preset_success_without_authoritative_catalog_fails_closed() {
    let (mut runner, mut runtime, mut host, request) = prepare_native_rename();
    let original_names = runner.preset_names.clone();

    let output = runtime
        .dispatch_host_message_music_first(
            save_result(request.request_id(), request.revision(), "Target"),
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(output.messages.iter().any(|message| matches!(message,
        RunnerMessage::Snapshot { snapshot }
            if snapshot["display"]["title"] == "RUNTIME ERROR"
    )));
    assert_eq!(runner.preset_names, original_names);
    assert_eq!(runner.current_preset_name, None);
    assert!(runner.pending.native_preset_write.is_none());
    assert!(host.effects.is_empty(), "{:?}", host.effects);
}

#[test]
fn cleanup_failure_uses_the_returned_catalog_and_honest_status() {
    let (mut runner, mut runtime, mut host, request) = prepare_native_rename();
    assert!(!runner.attach_native_preset_catalog(
        request.request_id(),
        request.revision(),
        vec!["Later".into(), "Other".into()],
        None,
    ));
    assert!(runner.attach_native_preset_catalog(
        request.request_id(),
        request.revision(),
        vec!["Other".into(), "Source".into(), "Target".into()],
        Some("source removal failed".into()),
    ));
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.synth.osc1.levelPct".into()),
        press_action: None,
    });
    let _ = dispatch_input(
        &mut runner,
        &mut runtime,
        &mut host,
        json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
    );
    assert_eq!(runner.config_revision, request.revision() + 1);

    let output = runtime
        .dispatch_host_message_music_first(
            save_result(request.request_id(), request.revision(), "Target"),
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert!(!output
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert_eq!(runner.preset_names, vec!["Other", "Source", "Target"]);
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Saved Target; cleanup failed: source removal failed")
    );
    assert!(host.effects.is_empty());
}
