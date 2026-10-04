#![cfg(not(feature = "hardware-orange-pi-zero-2w"))]

use super::*;
use playback_runtime::{RunnerMessage, RuntimePlatformEffect, RuntimeTransportState};
use std::sync::mpsc;

fn input(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
    value: serde_json::Value,
) {
    crate::runtime_loop::dispatch_runtime_message(
        playback,
        runner,
        host,
        HostMessage::DeviceInput {
            input: value,
            request_snapshot: Some(false),
        },
    )
    .unwrap();
}

fn confirm_action(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
    key: &str,
) {
    runner.test_focus_menu_item(key).unwrap();
    for message in [
        json!({"type":"encoder_press","id":"main"}),
        json!({"type":"encoder_turn","id":"main","delta":1}),
        json!({"type":"encoder_press","id":"main"}),
    ] {
        input(playback, runner, host, message);
    }
}

fn next_preset_result(
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
) -> (String, String) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        for message in host.drain_platform_results_for_runner(runner, 8) {
            let saved = match &message {
                HostMessage::RuntimeResult {
                    result:
                        RuntimeStoreResult::Identified {
                            result, request_id, ..
                        },
                } if request_id.starts_with("native-preset-") => match result.as_ref() {
                    RuntimeStoreResult::SavePresetResult { name, outcome } => {
                        Some((name.clone(), outcome.clone()))
                    }
                    RuntimeStoreResult::RuntimeFailure { error } => {
                        panic!("native preset save failed: {error:?}")
                    }
                    _ => None,
                },
                _ => None,
            };
            playback
                .dispatch_host_message_music_first(message, runner, host)
                .unwrap();
            if let Some(saved) = saved {
                return saved;
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("native preset save did not complete");
}

fn start_playing(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
) {
    runner.skip_startup_splash();
    for value in [
        json!({"type":"button_s","pressed":true}),
        json!({"type":"button_s","pressed":false}),
        json!({"type":"grid_press","x":2,"y":3}),
        json!({"type":"grid_release","x":2,"y":3}),
    ] {
        input(playback, runner, host, value);
    }
    assert_eq!(
        playback.last_status().unwrap().transport,
        RuntimeTransportState::Playing
    );
}

fn gate_worker(host: &crate::host_adapter::PiPlaybackHostAdapter) -> mpsc::Sender<()> {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    host.core
        .platform_service
        .enqueue(crate::platform_service::PlatformJob::new(
            RuntimePlatformRequest::new(
                RuntimePlatformEffect::SystemInfoRequest,
                "preset-input-gate".into(),
                None,
            ),
            crate::platform_service::PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    release_tx
}

fn keep_playing_while_save_is_queued(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
) {
    input(
        playback,
        runner,
        host,
        json!({"type":"grid_press","x":4,"y":3}),
    );
    input(
        playback,
        runner,
        host,
        json!({"type":"grid_release","x":4,"y":3}),
    );
    HostAdapter::handle_audio_command(
        host,
        &RuntimeAudioCommand::SetMasterVolume {
            generation: 0,
            volume_pct: 75.0,
        },
    )
    .unwrap();
    HostAdapter::handle_midi_message(host, &[0x90, 60, 100]).unwrap();
    for _ in 0..4 {
        let source = playback.config().sync_source.clone();
        playback
            .dispatch_host_message_music_first(
                HostMessage::TransportPulseStep {
                    pulses: 1,
                    source,
                    at_ppqn_pulse: None,
                    request_snapshot: Some(false),
                },
                runner,
                host,
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(8));
    }
    assert_eq!(
        playback.last_status().unwrap().transport,
        RuntimeTransportState::Playing
    );
}

fn save_current(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
) -> (String, String) {
    confirm_action(playback, runner, host, "preset.saveCurrent");
    next_preset_result(host, playback, runner)
}

fn stop_playing(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
) {
    for value in [
        json!({"type":"button_shift","pressed":true}),
        json!({"type":"button_s","pressed":true}),
        json!({"type":"button_s","pressed":false}),
        json!({"type":"button_shift","pressed":false}),
    ] {
        input(playback, runner, host, value);
    }
    assert_eq!(
        playback.last_status().unwrap().transport,
        RuntimeTransportState::Stopped
    );
}

fn edit_rename_target(
    source_name: &str,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
) {
    runner
        .test_focus_menu_item(&format!("preset.renamePick.{source_name}"))
        .unwrap();
    input(
        playback,
        runner,
        host,
        json!({"type":"encoder_press","id":"main"}),
    );
    refresh_preset_catalog(host, playback, runner);
    runner
        .test_focus_menu_item(&format!("preset.renamePick.{source_name}"))
        .unwrap();
    for _ in 0..8 {
        if runner.test_current_menu_label().as_deref() == Some("New Name") {
            break;
        }
        input(
            playback,
            runner,
            host,
            json!({"type":"encoder_turn","id":"main","delta":1}),
        );
    }
    assert_eq!(
        runner.test_current_menu_label().as_deref(),
        Some("New Name")
    );
    input(
        playback,
        runner,
        host,
        json!({"type":"encoder_press","id":"main"}),
    );
    input(
        playback,
        runner,
        host,
        json!({"type":"encoder_turn","id":"main","delta":1}),
    );
    input(
        playback,
        runner,
        host,
        json!({"type":"button_a","pressed":true}),
    );
    std::thread::sleep(Duration::from_millis(25));
    let responses = runner.flush_deferred_menu_apply().unwrap();
    let output = playback
        .dispatch_runner_messages(responses, runner, host)
        .unwrap();
    crate::runtime_loop::process_runtime_output(playback, runner, host, output).unwrap();
}

fn save_as_with_held_worker(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
) -> (String, serde_json::Value) {
    let expected = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    let release = gate_worker(host);
    confirm_action(playback, runner, host, "preset.saveAs.save");
    keep_playing_while_save_is_queued(playback, runner, host);
    release.send(()).unwrap();
    let (name, outcome) = next_preset_result(host, playback, runner);
    assert_eq!(outcome, "created");
    (name, expected)
}

fn check_catalog_rows(runner: &mut NativeRunner, name: &str) {
    for key in [
        format!("preset.load.{name}"),
        format!("preset.renamePick.{name}"),
        format!("preset.delete.{name}"),
    ] {
        assert_eq!(runner.test_focus_menu_item(&key).unwrap(), name);
    }
    for (key, label) in [
        ("preset.library.load", "Load"),
        ("preset.library.rename", "Rename"),
        ("preset.library.delete", "Delete"),
    ] {
        assert_eq!(runner.test_focus_menu_item(key).unwrap(), label);
    }
}

fn receive_partial_rename(
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    source_name: &str,
) -> (String, String, String) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        for result in host.core.platform_service.drain_platform_results(8) {
            match result {
                PlatformResult::NativePresetCompletion(completion) => {
                    let cleanup_error = completion.cleanup_error.clone().unwrap();
                    assert!(cleanup_error.contains(source_name));
                    assert!(cleanup_error.contains("does not exist"));
                    let message = finish_platform_result(
                        &host.core.platform_service,
                        runner,
                        PlatformResult::NativePresetCompletion(completion),
                    )
                    .expect("matching rename completion should be dispatched");
                    let (name, outcome) = match &message {
                        HostMessage::RuntimeResult {
                            result: RuntimeStoreResult::Identified { result, .. },
                        } => match result.as_ref() {
                            RuntimeStoreResult::SavePresetResult { name, outcome } => {
                                (name.clone(), outcome.clone())
                            }
                            result => panic!("expected successful partial rename, got {result:?}"),
                        },
                        message => panic!("expected preset result, got {message:?}"),
                    };
                    playback
                        .dispatch_host_message_music_first(message, runner, host)
                        .unwrap();
                    return (name, outcome, cleanup_error);
                }
                result => {
                    if let Some(message) =
                        finish_platform_result(&host.core.platform_service, runner, result)
                    {
                        if let HostMessage::RuntimeResult {
                            result: RuntimeStoreResult::Identified { result, .. },
                        } = &message
                        {
                            if let RuntimeStoreResult::RuntimeFailure { error } = result.as_ref() {
                                panic!("native rename worker failed: {error:?}");
                            }
                        }
                        playback
                            .dispatch_host_message_music_first(message, runner, host)
                            .unwrap();
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("partial rename completion did not arrive");
}

fn cleanup_warning_reaches_display(runner: &mut NativeRunner) {
    for _ in 0..100 {
        let snapshot = runner
            .messages_with_snapshot()
            .unwrap()
            .into_iter()
            .find_map(|message| match message {
                RunnerMessage::Snapshot { snapshot } => Some(snapshot),
                _ => None,
            })
            .unwrap();
        if snapshot["display"]["toast"]
            .as_str()
            .unwrap()
            .contains("cleanup")
        {
            return;
        }
    }
    panic!("cleanup warning never scrolled into view");
}

fn rename_after_source_disappears(
    source_name: &str,
    source_path: &std::path::Path,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
) -> (String, String, serde_json::Value) {
    stop_playing(playback, runner, host);
    edit_rename_target(source_name, playback, runner, host);
    start_playing(playback, runner, host);
    let expected = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    let release = gate_worker(host);
    confirm_action(playback, runner, host, "preset.rename.apply");
    assert!(matches!(
        host.core.platform_service.native_preset_write().unwrap().manual,
        NativeManualSaveRequest::Preset {
            mode: None,
            rename_from: Some(ref source),
            ..
        } if source == source_name
    ));
    assert!(source_path.is_file());
    std::fs::remove_file(source_path).unwrap();
    release.send(()).unwrap();
    let (name, outcome, cleanup_error) =
        receive_partial_rename(host, playback, runner, source_name);
    assert!(!source_path.exists());
    cleanup_warning_reaches_display(runner);
    assert!(cleanup_error.contains(source_name));
    (name, outcome, expected)
}

#[test]
fn playing_save_as_save_current_and_rename_follow_device_input_and_refresh_catalog() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-pi-preset-input");
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let defaults: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    let documents = playback_runtime::split_system_patch_documents(&defaults).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    let mut host = crate::host_adapter::PiPlaybackHostAdapter::new(
        None,
        store,
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        crate::usb_config::UsbAudioOut::Jack,
    );
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    start_playing(&mut playback, &mut runner, &mut host);
    let (source_name, expected_patch) =
        save_as_with_held_worker(&mut playback, &mut runner, &mut host);
    let source_path = root
        .join("store/patches")
        .join(format!("{source_name}.json"));
    assert_eq!(
        crate::platform_service::load_json(&source_path).unwrap(),
        Some(expected_patch)
    );
    check_catalog_rows(&mut runner, &source_name);
    let (saved_current, current_outcome) = save_current(&mut playback, &mut runner, &mut host);
    assert_eq!(saved_current, source_name);
    assert_eq!(current_outcome, "overwritten");

    let (target_name, outcome, expected_rename_patch) = rename_after_source_disappears(
        &source_name,
        &source_path,
        &mut playback,
        &mut runner,
        &mut host,
    );
    assert_ne!(target_name, source_name);
    assert_eq!(outcome, "created");
    assert!(!source_path.exists());
    assert!(root
        .join("store/patches")
        .join(format!("{target_name}.json"))
        .is_file());
    assert_eq!(
        crate::platform_service::load_json(
            &crate::platform_service::preset_patch_path(&root.join("store"), &target_name,)
                .unwrap(),
        )
        .unwrap(),
        Some(expected_rename_patch)
    );
    assert_eq!(
        runner
            .test_focus_menu_item(&format!("preset.load.{target_name}"))
            .unwrap(),
        target_name
    );
    assert!(runner
        .test_focus_menu_item(&format!("preset.renamePick.{source_name}"))
        .is_err());
    let (current_name, current_outcome) = save_current(&mut playback, &mut runner, &mut host);
    assert_eq!(current_name, target_name);
    assert_eq!(current_outcome, "overwritten");
    assert!(host.core.platform_service.native_preset_write().is_none());
    let _ = std::fs::remove_dir_all(root);
}
