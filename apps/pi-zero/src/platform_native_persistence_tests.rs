#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use super::*;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::host_adapter::PiPlaybackHostAdapter;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::usb_config::UsbAudioOut;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use playback_runtime::NativeRunnerConfig;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use playback_runtime::{
    HostMessage, PlaybackRuntime, RuntimeConfig, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult,
};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use serde_json::json;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::sync::mpsc;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::time::{Duration, Instant};

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn adapter() -> (PiPlaybackHostAdapter, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-native-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let adapter = PiPlaybackHostAdapter::new(
        None,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Jack,
    );
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
    std::fs::write(
        crate::platform_service::default_patch_path(&store),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    (adapter, root)
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn cleanup(root: std::path::PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn malformed_current_completions_clear_pending_and_stale_success_cannot_clear_retry() {
    use playback_runtime::RuntimeOperation;

    let (mut adapter, root) = adapter();
    let store = root.join("store");
    let prior_patch_bytes =
        std::fs::read(crate::platform_service::default_patch_path(&store)).unwrap();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    adapter
        .platform_service
        .enqueue(crate::platform_service::PlatformJob::new(
            RuntimePlatformRequest::new(
                RuntimePlatformEffect::SystemInfoRequest,
                "gate".into(),
                Some(0),
            ),
            crate::platform_service::PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();

    let mut requests = Vec::new();
    for malformed_kind in 0..3 {
        let snapshot = runner.capture_config_snapshot();
        let request = playback
            .next_native_store_request(RuntimeOperation::StoreSaveDefault, snapshot.revision());
        assert!(
            crate::platform_service::platform_native_persistence::submit_native_default(
                &adapter.platform_service,
                &mut runner,
                request.clone(),
                snapshot,
            )
            .is_none()
        );
        let (completion_revision, prepared) = match malformed_kind {
            0 => (Some(request.revision()), None),
            1 => (
                Some(request.revision()),
                Some(Arc::new(json!({"revision": 999}))),
            ),
            _ => (
                None,
                Some(Arc::new(json!({"revision": request.revision()}))),
            ),
        };
        let failure = finish_platform_result(
            &adapter.platform_service,
            &mut runner,
            PlatformResult::NativeDefaultCompletion(NativeDefaultCompletion {
                result: RuntimeStoreResult::Identified {
                    result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                        ok: true,
                        is_auto: None,
                    }),
                    request_id: request.request_id().into(),
                    revision: completion_revision,
                },
                prepared,
            }),
        )
        .expect("current malformed completion should fail visibly");
        assert!(matches!(
            &failure,
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::Identified {
                    request_id,
                    revision: Some(revision),
                    result,
                }
            } if request_id == request.request_id()
                && *revision == request.revision()
                && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
                    if error.operation == RuntimeOperation::StoreSaveDefault)
        ));
        playback
            .dispatch_host_message_music_first(failure, &mut runner, &mut adapter)
            .unwrap();
        assert!(!playback.latched_errors().is_empty());
        assert!(adapter.platform_service.native_default_write().is_none());
        assert_eq!(
            std::fs::read(crate::platform_service::default_patch_path(&store)).unwrap(),
            prior_patch_bytes
        );
        requests.push(request);
    }

    let snapshot = runner.capture_config_snapshot();
    let latest =
        playback.next_native_store_request(RuntimeOperation::StoreSaveDefault, snapshot.revision());
    assert!(
        crate::platform_service::platform_native_persistence::submit_native_default(
            &adapter.platform_service,
            &mut runner,
            latest.clone(),
            snapshot,
        )
        .is_none()
    );
    let stale_success = PlatformResult::NativeDefaultCompletion(NativeDefaultCompletion {
        result: RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: None,
            }),
            request_id: requests[0].request_id().into(),
            revision: Some(requests[0].revision()),
        },
        prepared: Some(Arc::new(json!({"revision": requests[0].revision()}))),
    });
    assert!(
        finish_platform_result(&adapter.platform_service, &mut runner, stale_success).is_none()
    );
    assert_eq!(
        adapter.platform_service.native_default_write(),
        Some(latest.clone())
    );
    assert!(!runner.register_native_default_write("stale-probe", latest.revision(), false,));

    release_tx.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saved = false;
    while Instant::now() < deadline && !saved {
        for message in adapter.drain_platform_results_for_runner(&mut runner, 8) {
            saved |= matches!(
                &message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { result, request_id, .. }
                } if request_id == latest.request_id()
                    && matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, .. })
            );
            playback
                .dispatch_host_message_music_first(message, &mut runner, &mut adapter)
                .unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(saved);
    let saved_patch = adapter
        .platform_service
        .load_default_now()
        .unwrap()
        .unwrap();
    assert_eq!(saved_patch["kind"], "octessera.patch");
    assert!(saved_patch.get("revision").is_none());
    cleanup(root);
}

#[test]
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn playing_manual_default_save_queues_before_worker_serialization_and_reloads() {
    use playback_runtime::RuntimeTransportState;

    let (mut adapter, root) = adapter();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.skip_startup_splash();
    for input in [
        json!({"type":"button_s","pressed":true}),
        json!({"type":"button_s","pressed":false}),
        json!({"type":"grid_press","x":2,"y":3}),
        json!({"type":"grid_release","x":2,"y":3}),
    ] {
        crate::runtime_loop::dispatch_runtime_message(
            &mut playback,
            &mut runner,
            &mut adapter,
            HostMessage::DeviceInput {
                input,
                request_snapshot: Some(false),
            },
        )
        .unwrap();
    }
    assert_eq!(
        playback.last_status().unwrap().transport,
        RuntimeTransportState::Playing
    );

    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    adapter
        .platform_service
        .enqueue(crate::platform_service::PlatformJob::new(
            RuntimePlatformRequest::new(
                RuntimePlatformEffect::SystemInfoRequest,
                "gate".into(),
                Some(0),
            ),
            crate::platform_service::PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();

    runner.test_focus_menu_item("default.save").unwrap();
    for input in [
        json!({"type":"encoder_press","id":"main"}),
        json!({"type":"encoder_turn","id":"main","delta":1}),
        json!({"type":"encoder_press","id":"main"}),
    ] {
        crate::runtime_loop::dispatch_runtime_message(
            &mut playback,
            &mut runner,
            &mut adapter,
            HostMessage::DeviceInput {
                input,
                request_snapshot: Some(false),
            },
        )
        .unwrap();
    }
    let source = playback.config().sync_source.clone();
    crate::runtime_loop::dispatch_runtime_message(
        &mut playback,
        &mut runner,
        &mut adapter,
        HostMessage::TransportPulseStep {
            pulses: 1,
            source,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        },
    )
    .unwrap();
    assert_eq!(
        playback.last_status().unwrap().transport,
        RuntimeTransportState::Playing
    );
    playback_runtime::HostAdapter::handle_audio_command(
        &mut adapter,
        &playback_runtime::RuntimeAudioCommand::SetMasterVolume {
            generation: 0,
            volume_pct: 71.0,
        },
    )
    .unwrap();
    release_tx.send(()).unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut completed = false;
    while Instant::now() < deadline && !completed {
        for message in adapter.drain_platform_results_for_runner(&mut runner, 4) {
            completed |= matches!(
                &message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { result, .. }
                } if matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, .. })
            );
            crate::runtime_loop::dispatch_runtime_message(
                &mut playback,
                &mut runner,
                &mut adapter,
                message,
            )
            .unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(completed, "native default save completion did not arrive");
    let loaded = adapter
        .platform_service
        .load_default_now()
        .unwrap()
        .unwrap();
    assert!(loaded.get("runtimeConfig").is_some());
    cleanup(root);
}
