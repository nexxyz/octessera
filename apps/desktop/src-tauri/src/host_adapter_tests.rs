use super::DesktopPlaybackHostAdapter;
use crate::types::QueuedAudioEvent;
use playback_runtime::{
    HostAdapter, HostMessage, RuntimeAudioCommand, RuntimePlatformEffect, RuntimeStoreResult,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

fn test_adapter() -> (DesktopPlaybackHostAdapter, mpsc::Receiver<QueuedAudioEvent>) {
    let (tx, rx) = mpsc::channel();
    let adapter = DesktopPlaybackHostAdapter {
        trigger_tx: tx,
        sample_cache: Arc::new(Mutex::new(HashMap::new())),
        midi_out: Arc::new(Mutex::new(None)),
        midi_in: Arc::new(Mutex::new(None)),
        midi_in_handler: Arc::new(|_| {}),
        store_dir: PathBuf::new(),
        pending_default_save: None,
        selected_midi_output_id: None,
        selected_midi_input_id: None,
        shutdown_requested: false,
    };
    (adapter, rx)
}

#[test]
fn platform_effect_audio_command_reaches_audio_queue() {
    let (mut adapter, rx) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::MomentaryFxStop {
                id: "preview".into(),
            },
        })
        .unwrap();
    assert!(follow_ups.is_empty());
    assert!(
        matches!(rx.try_recv().unwrap(), QueuedAudioEvent::MomentaryFxStop { id } if id == "preview")
    );
}

#[test]
fn deferred_default_save_flushes_runtime_result() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = std::env::temp_dir().join(format!(
        "cellsymphony-host-adapter-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    adapter.store_dir = temp_dir.clone();
    let payload = serde_json::json!({ "runtimeConfig": { "masterVolume": 73 } });
    let follow_ups = adapter
        .handle_platform_effect(&RuntimePlatformEffect::StoreSaveDefault {
            payload: payload.clone(),
            mode: Some("deferred".into()),
        })
        .unwrap();
    assert!(follow_ups.is_empty());
    adapter.pending_default_save = adapter
        .pending_default_save
        .take()
        .map(|(payload, _)| (payload, Instant::now()));
    let follow_ups = adapter.flush_due_default_save().unwrap();
    assert_eq!(
        follow_ups,
        vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true)
            }
        }]
    );
    assert!(temp_dir.join("default.json").is_file());
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn malformed_default_load_returns_store_error() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = std::env::temp_dir().join(format!(
        "cellsymphony-host-adapter-bad-default-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::write(temp_dir.join("default.json"), "not json").unwrap();
    adapter.store_dir = temp_dir.clone();
    let follow_ups = adapter
        .handle_platform_effect(&RuntimePlatformEffect::StoreLoadDefault)
        .unwrap();
    assert!(
        matches!(&follow_ups[..], [HostMessage::RuntimeResult { result: RuntimeStoreResult::StoreError { message } }] if message.starts_with("Default load failed:"))
    );
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn midi_panic_returns_native_status_result() {
    let (mut adapter, _) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&RuntimePlatformEffect::MidiPanic)
        .unwrap();
    assert_eq!(
        follow_ups,
        vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiStatus {
                ok: true,
                message: Some("Panic sent".into()),
                selected_out_id: None,
                selected_in_id: None
            }
        }]
    );
}

#[test]
fn shutdown_effect_sets_pending_shutdown_request() {
    let (mut adapter, _) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&RuntimePlatformEffect::Shutdown)
        .unwrap();
    assert!(follow_ups.is_empty());
    assert!(adapter.take_shutdown_request());
    assert!(!adapter.take_shutdown_request());
}
