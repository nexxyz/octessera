use super::*;
use crate::audio::test_service;
use playback_runtime::{
    CoreRunner, HostAdapter, NativeRunner, NativeRunnerConfig, PlaybackRuntime,
    RuntimeAudioCommand, RuntimeConfig, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult, RuntimeTransportState, RuntimeUserDataRestorePhase,
    RuntimeUserDataRestoreStatus,
};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[path = "orange_host_adapter_drain_tests.rs"]
mod drain_tests;
#[path = "orange_host_adapter_media_tests.rs"]
mod media_tests;
#[path = "orange_host_adapter_setup_portal_tests.rs"]
mod setup_portal_tests;

fn directories() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "octessera-orange-host-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    (root.join("store"), root.join("samples"))
}

fn adapter() -> (OrangeHostAdapter, PathBuf, PathBuf) {
    let (store, samples) = directories();
    let (audio, _, _) = test_service();
    let documents = playback_runtime::split_system_patch_documents(
        &crate::user_data_archive::canonical_defaults(),
    )
    .unwrap();
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store.join("default.patch.json"),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    let adapter = OrangeHostAdapter::with_directories(
        audio,
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    (adapter, store, samples)
}

fn request(effect: RuntimePlatformEffect, id: &str) -> RuntimePlatformRequest {
    RuntimePlatformRequest::new(effect, id.into(), Some(1))
}

fn wait_for_result(adapter: &OrangeHostAdapter) -> Vec<HostMessage> {
    adapter
        .core
        .platform_service
        .enqueue_test_barrier()
        .unwrap()
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    adapter.drain_results(4)
}

fn unwrap_result(message: HostMessage) -> RuntimeStoreResult {
    let HostMessage::RuntimeResult { result } = message else {
        panic!("expected runtime result");
    };
    match result {
        RuntimeStoreResult::Identified { result, .. } => *result,
        result => result,
    }
}

#[test]
fn first_backup_effect_does_not_latch_an_error() {
    let (mut adapter, store, samples) = adapter();
    let patch = crate::platform_service::load_json(&store.join("default.patch.json"))
        .unwrap()
        .unwrap();
    let response = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreSaveBackup { payload: patch },
            "backup-1",
        ))
        .unwrap();
    assert!(response.is_empty());
    let _ = wait_for_result(&adapter);
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn default_and_preset_round_trips_use_atomic_service() {
    let (mut adapter, store, samples) = adapter();
    let mut full = crate::user_data_archive::canonical_defaults();
    full["runtimeConfig"]["bpm"] = json!(99);
    let payload = playback_runtime::split_system_patch_documents(&full)
        .unwrap()
        .patch;
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreSaveDefault {
                payload: payload.clone(),
                mode: None,
            },
            "default-save",
        ))
        .unwrap()
        .is_empty());
    let _ = wait_for_result(&adapter);
    let loaded = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreLoadDefault,
            "default-load",
        ))
        .unwrap();
    assert!(matches!(
        loaded.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(value)
            }
        }] if value == &payload
    ));

    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreSavePreset {
                name: "round-trip".into(),
                payload: payload.clone(),
                mode: None,
            },
            "preset-save",
        ))
        .unwrap()
        .is_empty());
    let save_result = unwrap_result(wait_for_result(&adapter).remove(0));
    assert!(matches!(
        save_result,
        RuntimeStoreResult::SavePresetResult { .. }
    ));
    let patch_path = store.join("patches").join("round-trip.json");
    assert!(patch_path.is_file());
    assert!(std::fs::read_dir(patch_path.parent().unwrap())
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")));
    let loaded = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreLoadPreset {
                name: "round-trip".into(),
            },
            "preset-load",
        ))
        .unwrap()
        .remove(0);
    let load_result = unwrap_result(loaded);
    assert!(matches!(
        load_result,
        RuntimeStoreResult::LoadPresetResult { payload: Some(value), .. }
            if value == payload
    ));
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn midi_panic_and_clear_selection_succeed_without_selected_ports() {
    let (store, samples) = directories();
    let (audio, _, _event_rx) = test_service();
    let mut adapter = OrangeHostAdapter::with_directories(
        audio,
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    for effect in [
        RuntimePlatformEffect::MidiPanic,
        RuntimePlatformEffect::MidiSelectOutput { id: None },
        RuntimePlatformEffect::MidiSelectInput { id: None },
    ] {
        let responses = adapter
            .handle_platform_effect(&request(effect, "midi"))
            .unwrap();
        let [HostMessage::RuntimeResult { result }] = responses.as_slice() else {
            panic!("expected MIDI status");
        };
        assert!(matches!(
            result,
            RuntimeStoreResult::MidiStatus { ok: true, .. }
        ));
    }
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn runtime_restore_loads_default_before_orange_barrier_acknowledgement() {
    let (mut adapter, store, samples) = adapter();
    let mut payload = crate::user_data_archive::canonical_defaults();
    payload["runtimeConfig"]["masterVolume"] = json!(81);
    let documents = playback_runtime::split_system_patch_documents(&payload).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store.join("default.patch.json"),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    adapter
        .core
        .platform_service
        .invalidate_store_writes_for_test();

    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let status_messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::UserDataRestoreStatus {
                status: RuntimeUserDataRestoreStatus {
                    phase: RuntimeUserDataRestorePhase::Succeeded,
                },
            }
            .with_identity("restore-e2e".into(), Some(3)),
        })
        .unwrap();
    assert!(adapter.core.platform_service.store_writes_blocked());
    playback
        .dispatch_runner_messages(status_messages, &mut runner, &mut adapter)
        .unwrap();

    assert!(!adapter.core.platform_service.store_writes_blocked());
    assert_eq!(
        playback.last_snapshot().unwrap()["settings"]["masterVolume"],
        81
    );
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn failed_runtime_restore_apply_keeps_orange_barrier_blocked() {
    let (mut adapter, store, samples) = adapter();
    let recovery = br#"{"recovery":true}"#;
    std::fs::write(
        store.join("default.patch.json"),
        br#"{"runtimeConfig":"bad"}"#,
    )
    .unwrap();
    std::fs::write(store.join("recovery-save.json"), recovery).unwrap();
    adapter
        .core
        .platform_service
        .invalidate_store_writes_for_test();

    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let status_messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::UserDataRestoreStatus {
                status: RuntimeUserDataRestoreStatus {
                    phase: RuntimeUserDataRestorePhase::Succeeded,
                },
            }
            .with_identity("restore-failed".into(), Some(4)),
        })
        .unwrap();
    playback
        .dispatch_runner_messages(status_messages, &mut runner, &mut adapter)
        .unwrap();

    assert!(adapter.core.platform_service.store_writes_blocked());
    assert_eq!(
        std::fs::read(store.join("recovery-save.json")).unwrap(),
        recovery
    );
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn orange_playing_save_as_uses_shared_worker_and_keeps_audio_pulses_live() {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let (store, samples) = directories();
    crate::pi_store_test_support::write_pair(
        &store,
        &crate::user_data_archive::canonical_defaults(),
    );
    let (audio, control_rx, mut event_rx, prep_tx) = crate::audio::test_service_with_prep_sender();
    let mut adapter = OrangeHostAdapter::with_directories(
        audio,
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
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
        crate::orange_candidate::dispatch(
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
    let expected = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    adapter
        .core
        .platform_service
        .enqueue(crate::platform_service::PlatformJob::new(
            request(
                RuntimePlatformEffect::SystemInfoRequest,
                "preset-worker-gate",
            ),
            PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();

    runner.test_focus_menu_item("preset.saveAs.save").unwrap();
    for input in [
        json!({"type":"encoder_press","id":"main"}),
        json!({"type":"encoder_turn","id":"main","delta":1}),
        json!({"type":"encoder_press","id":"main"}),
    ] {
        crate::orange_candidate::dispatch(
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
    for _ in 0..4 {
        let source = playback.config().sync_source.clone();
        crate::orange_candidate::dispatch(
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
        std::thread::sleep(Duration::from_millis(8));
    }
    let audio_events = std::iter::from_fn(|| event_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(audio_events
        .iter()
        .any(|event| matches!(event, rodio_engine_source::EngineEvent::NoteOn { .. })));
    HostAdapter::handle_audio_command(
        &mut adapter,
        &RuntimeAudioCommand::SetMasterVolume {
            generation: 0,
            volume_pct: 77.0,
        },
    )
    .unwrap();
    HostAdapter::handle_midi_message(&mut adapter, &[0x90, 60, 100]).unwrap();
    assert_eq!(
        playback.last_status().unwrap().transport,
        RuntimeTransportState::Playing
    );
    release_tx.send(()).unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saved_name = None;
    while Instant::now() < deadline && saved_name.is_none() {
        for message in adapter.drain_results_for_runner(&mut runner, 8) {
            if let HostMessage::RuntimeResult {
                result:
                    RuntimeStoreResult::Identified {
                        result, request_id, ..
                    },
            } = &message
            {
                if request_id.starts_with("native-preset-") {
                    if let RuntimeStoreResult::SavePresetResult { name, .. } = result.as_ref() {
                        saved_name = Some(name.clone());
                    }
                }
            }
            crate::orange_candidate::dispatch(&mut playback, &mut runner, &mut adapter, message)
                .unwrap();
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let name = saved_name.expect("Orange native preset save did not complete");
    let saved = crate::platform_service::load_json(
        &crate::platform_service::preset_patch_path(&store, &name).unwrap(),
    )
    .unwrap();
    assert_eq!(saved, Some(expected));
    assert!(runner
        .test_focus_menu_item(&format!("preset.renamePick.{name}"))
        .is_ok());
    drop(control_rx);
    drop(prep_tx);
    let _ = event_rx.try_recv();
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[path = "orange_host_adapter_parity_tests.rs"]
mod parity_tests;
