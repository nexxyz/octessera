use super::*;
use playback_runtime::{
    DrumHit, HostAdapter, HostMessage, MusicalEvent, NativeRunnerConfig, RuntimeAdapterError,
    RuntimeAudioCommand, RuntimePlatformRequest,
};
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "platform_native_preset_input_tests.rs"]
mod input_tests;
#[cfg(windows)]
#[path = "platform_native_preset_io_tests.rs"]
mod io_tests;
#[path = "platform_native_preset_local_sample_tests.rs"]
mod local_sample_tests;
#[cfg(test)]
#[path = "platform_native_preset_result_tests.rs"]
mod result_tests;

#[derive(Default)]
struct TestHost;

impl HostAdapter for TestHost {
    fn handle_musical_event(&mut self, _event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_drum_hit(&mut self, _hit: &DrumHit) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        _request: &RuntimePlatformRequest,
    ) -> Result<Vec<playback_runtime::HostMessage>, RuntimeAdapterError> {
        Ok(Vec::new())
    }

    fn handle_audio_command(
        &mut self,
        _command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
}

fn service_and_root(label: &str) -> (crate::platform_service::PiPlatformService, PathBuf) {
    let root = crate::test_temp_dir::unique_temp_path(&format!("octessera-native-preset-{label}"));
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let full: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    let documents = playback_runtime::split_system_patch_documents(&full).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    let service = crate::platform_service::PiPlatformService::new(store, root.join("samples"));
    (service, root)
}

fn enqueue_preset(
    service: &crate::platform_service::PiPlatformService,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    manual: NativeManualSaveRequest,
) -> NativeStoreRequest {
    let snapshot = runner.capture_config_snapshot();
    let request =
        playback.next_native_store_request(RuntimeOperation::StoreSavePreset, snapshot.revision());
    assert!(submit_native_preset(service, runner, request.clone(), snapshot, manual).is_none());
    request
}

fn receive_preset_result(
    service: &crate::platform_service::PiPlatformService,
    runner: &mut NativeRunner,
    timeout: Duration,
) -> playback_runtime::HostMessage {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        for result in service.drain_platform_results(8) {
            if let Some(message) = finish_platform_result(service, runner, result) {
                return message;
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("native preset result did not arrive");
}

fn write_patch(store_dir: &std::path::Path, name: &str, payload: &serde_json::Value) -> Vec<u8> {
    let path = crate::platform_service::preset_patch_path(store_dir, name).unwrap();
    crate::platform_service::save_json(&path, payload).unwrap();
    std::fs::read(path).unwrap()
}

fn cleanup(root: PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn refresh_preset_catalog(
    host: &mut crate::host_adapter::PiPlaybackHostAdapter,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
) {
    runner.test_focus_menu_item("preset.refresh").unwrap();
    crate::runtime_loop::dispatch_runtime_message(
        playback,
        runner,
        host,
        HostMessage::DeviceInput {
            input: json!({"type":"encoder_press","id":"main"}),
            request_snapshot: Some(false),
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        for message in host.drain_platform_results_for_runner(runner, 8) {
            let listed = matches!(
                &message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { result, .. }
                } if matches!(result.as_ref(), RuntimeStoreResult::ListPresetsResult { .. })
            );
            playback
                .dispatch_host_message_music_first(message, runner, host)
                .unwrap();
            if listed {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("preset refresh did not complete");
}

fn wait_for_worker_queue(service: &crate::platform_service::PiPlatformService) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match service.enqueue_test_barrier() {
            Ok(barrier) => {
                barrier.recv_timeout(Duration::from_secs(1)).unwrap();
                return;
            }
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("preset worker queue did not drain: {error}"),
        }
    }
}

#[test]
fn playing_rename_overwrites_existing_target_before_removing_source() {
    let (service, root) = service_and_root("rename-target-overwrite");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let patch = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    write_patch(&store, "Source", &patch);
    write_patch(&store, "Target", &json!({"preserve": true}));
    let request = enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "Target".into(),
            mode: None,
            rename_from: Some("Source".into()),
        },
    );
    let message = receive_preset_result(&service, &mut runner, Duration::from_secs(2));
    let HostMessage::RuntimeResult {
        result:
            RuntimeStoreResult::Identified {
                result,
                request_id,
                revision: Some(revision),
            },
    } = &message
    else {
        panic!("preset overwrite must return an identified result");
    };
    assert_eq!(request_id, request.request_id());
    assert_eq!(*revision, request.revision());
    assert!(matches!(
        result.as_ref(),
        RuntimeStoreResult::SavePresetResult { name, outcome }
            if name == "Target" && outcome == "overwritten"
    ));
    assert!(!store.join("patches/Source.json").exists());
    assert_eq!(
        crate::platform_service::load_json(
            &crate::platform_service::preset_patch_path(&store, "Target").unwrap()
        )
        .unwrap(),
        Some(patch)
    );
    assert_eq!(
        crate::platform_service::list_presets(&store).unwrap(),
        vec!["Target"]
    );
    playback
        .dispatch_host_message_music_first(message, &mut runner, &mut TestHost)
        .unwrap();
    assert!(service.native_preset_write().is_none());
    cleanup(root);
}

#[test]
fn native_rename_to_default_name_updates_catalog_and_current_preset() {
    let (service, root) = service_and_root("rename-default-stem");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let expected = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    write_patch(&store, "Source", &expected);
    let request = enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "default".into(),
            mode: None,
            rename_from: Some("Source".into()),
        },
    );
    let message = receive_preset_result(&service, &mut runner, Duration::from_secs(2));
    assert!(matches!(
        &message,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, result, .. }
        } if request_id == request.request_id()
            && matches!(result.as_ref(), RuntimeStoreResult::SavePresetResult { name, .. }
                if name == "default")
    ));
    assert_eq!(
        crate::platform_service::list_presets(&store).unwrap(),
        vec!["default"]
    );
    assert!(!store.join("patches/Source.json").exists());
    assert_eq!(
        crate::platform_service::load_json(
            &crate::platform_service::preset_patch_path(&store, "default").unwrap()
        )
        .unwrap(),
        Some(expected)
    );
    playback
        .dispatch_host_message_music_first(message, &mut runner, &mut TestHost)
        .unwrap();
    assert_eq!(
        runner.test_focus_menu_item("preset.load.default").unwrap(),
        "default"
    );
    runner.test_focus_menu_item("preset.saveCurrent").unwrap();
    playback
        .dispatch_host_message_music_first(
            HostMessage::DeviceInput {
                input: json!({"type":"encoder_press","id":"main"}),
                request_snapshot: Some(false),
            },
            &mut runner,
            &mut TestHost,
        )
        .unwrap();
    assert!(runner.test_confirmation_is_open());
    cleanup(root);
}

#[test]
fn preset_queue_rejection_and_restore_generation_never_delete_rename_source() {
    use playback_runtime::{RuntimePlatformEffect, RuntimePlatformRequest};
    use std::sync::mpsc;

    let (service, root) = service_and_root("rename-queue-rejection");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let patch = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    let source_bytes = write_patch(&store, "Source", &patch);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    service
        .enqueue(crate::platform_service::PlatformJob::new(
            RuntimePlatformRequest::new(
                RuntimePlatformEffect::SystemInfoRequest,
                "preset-gate".into(),
                None,
            ),
            crate::platform_service::PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    for index in 0..32 {
        service
            .enqueue(crate::platform_service::PlatformJob::new(
                RuntimePlatformRequest::new(
                    RuntimePlatformEffect::SystemInfoRequest,
                    format!("preset-fill-{index}"),
                    None,
                ),
                crate::platform_service::PlatformJobKind::TestBarrier {
                    completed: mpsc::sync_channel(1).0,
                },
            ))
            .unwrap();
    }
    let snapshot = runner.capture_config_snapshot();
    let request =
        playback.next_native_store_request(RuntimeOperation::StoreSavePreset, snapshot.revision());
    let failed = submit_native_preset(
        &service,
        &mut runner,
        request,
        snapshot,
        NativeManualSaveRequest::Preset {
            name: "Target".into(),
            mode: None,
            rename_from: Some("Source".into()),
        },
    )
    .expect("full queue must reject preset rename");
    assert!(matches!(
        &failed,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, .. }
        } if matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
            if error.operation == RuntimeOperation::StoreSavePreset)
    ));
    playback
        .dispatch_host_message_music_first(failed, &mut runner, &mut TestHost)
        .unwrap();
    assert!(service.native_preset_write().is_none());
    assert_eq!(
        std::fs::read(store.join("patches/Source.json")).unwrap(),
        source_bytes
    );
    assert!(!store.join("patches/Target.json").exists());
    release_tx.send(()).unwrap();
    wait_for_worker_queue(&service);

    let request = enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "Target".into(),
            mode: None,
            rename_from: Some("Source".into()),
        },
    );
    let success = receive_preset_result(&service, &mut runner, Duration::from_secs(2));
    assert!(matches!(
        &success,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, result, .. }
        } if request_id == request.request_id()
            && matches!(result.as_ref(), RuntimeStoreResult::SavePresetResult { name, .. }
                if name == "Target")
    ));
    playback
        .dispatch_host_message_music_first(success, &mut runner, &mut TestHost)
        .unwrap();
    assert!(!store.join("patches/Source.json").exists());
    cleanup(root);
}

#[test]
fn restore_generation_cancels_queued_preset_rename_before_projection_or_delete() {
    use playback_runtime::RuntimeOperation;
    use std::time::Duration;

    let (service, root) = service_and_root("rename-restore-barrier");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let patch = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    let source_bytes = write_patch(&store, "Source", &patch);
    let store_guard = service.store_lock.lock().unwrap();
    let request = enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "Target".into(),
            mode: None,
            rename_from: Some("Source".into()),
        },
    );
    assert_eq!(request.operation(), &RuntimeOperation::StoreSavePreset);
    service.invalidate_store_writes_for_test();
    drop(store_guard);
    service
        .enqueue_test_barrier()
        .unwrap()
        .recv_timeout(Duration::from_secs(1))
        .unwrap();

    let cancelled = receive_preset_result(&service, &mut runner, Duration::from_millis(10));
    assert!(matches!(
        &cancelled,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, result, .. }
        } if request_id == request.request_id()
            && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { .. })
    ));
    playback
        .dispatch_host_message_music_first(cancelled, &mut runner, &mut TestHost)
        .unwrap();
    assert!(service.native_preset_write().is_none());
    assert_eq!(
        std::fs::read(store.join("patches/Source.json")).unwrap(),
        source_bytes
    );
    assert!(!store.join("patches/Target.json").exists());
    cleanup(root);
}
