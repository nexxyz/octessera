use super::{DesktopHostAudioState, DesktopPlaybackHostAdapter};
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
    let (platform_service_tx, _) = mpsc::channel();
    let adapter = DesktopPlaybackHostAdapter {
        audio: DesktopHostAudioState {
            trigger_tx: tx,
            synth_slots: Arc::new(Mutex::new(
                [true; realtime_engine::synth::INSTRUMENT_SLOT_COUNT],
            )),
            sample_cache: Arc::new(Mutex::new(HashMap::new())),
            sample_bank_signature: Arc::new(Mutex::new(String::new())),
        },
        midi_out: Arc::new(Mutex::new(None)),
        midi_in: Arc::new(Mutex::new(None)),
        midi_in_handler: Arc::new(|_| {}),
        store_dir: PathBuf::new(),
        pending_default_save: None,
        platform_service_tx,
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
fn sample_list_request_reports_service_unavailable_when_enqueue_fails() {
    let (mut adapter, _) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&RuntimePlatformEffect::SampleListRequest {
            instrument_slot: 1,
            sample_slot: 2,
            dir: "kits".into(),
        })
        .unwrap();

    assert!(
        matches!(&follow_ups[..], [HostMessage::RuntimeResult { result: RuntimeStoreResult::SampleListError { instrument_slot: 1, sample_slot: 2, dir, message } }] if dir == "kits" && message == "Desktop platform service unavailable")
    );
}

#[test]
fn full_audio_config_command_sets_instruments_and_sample_banks() {
    let (mut adapter, rx) = test_adapter();
    let follow_ups = adapter
        .handle_platform_effect(&RuntimePlatformEffect::AudioCommand {
            command: RuntimeAudioCommand::SetAudioConfig {
                revision: 7,
                config: serde_json::json!({
                    "masterVolume": 82,
                    "voiceStealingMode": "auto-hard",
                    "panPositions": 33,
                    "instruments": [{
                        "type": "synth",
                        "mixer": { "route": "direct", "panPos": 16, "volume": 77 }
                    }],
                    "mixer": { "buses": [], "master": { "slots": [] } }
                }),
            },
        })
        .unwrap();

    assert!(follow_ups.is_empty());
    assert!(
        matches!(rx.try_recv().unwrap(), QueuedAudioEvent::SetInstruments(config) if config.master_volume == 82.0)
    );
    assert!(matches!(
        rx.try_recv().unwrap(),
        QueuedAudioEvent::SetSampleBanks(_)
    ));
    assert!(matches!(
        rx.try_recv().unwrap(),
        QueuedAudioEvent::SetVoiceStealingMode(realtime_engine::synth::VoiceStealingMode::AutoHard)
    ));
}

#[test]
fn full_audio_config_command_reuses_sample_bank_signature() {
    let (mut adapter, rx) = test_adapter();
    let command = RuntimePlatformEffect::AudioCommand {
        command: RuntimeAudioCommand::SetAudioConfig {
            revision: 7,
            config: serde_json::json!({
                "masterVolume": 82,
                "panPositions": 33,
                "instruments": [{ "type": "synth" }],
                "mixer": { "buses": [], "master": { "slots": [] } }
            }),
        },
    };

    adapter.handle_platform_effect(&command).unwrap();
    assert!(matches!(
        rx.try_recv().unwrap(),
        QueuedAudioEvent::SetInstruments(_)
    ));
    assert!(matches!(
        rx.try_recv().unwrap(),
        QueuedAudioEvent::SetSampleBanks(_)
    ));

    adapter.handle_platform_effect(&command).unwrap();
    assert!(matches!(
        rx.try_recv().unwrap(),
        QueuedAudioEvent::SetInstruments(_)
    ));
    assert!(rx.try_recv().is_err());
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
fn pending_default_save_flushes_immediately_on_shutdown() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = std::env::temp_dir().join(format!(
        "cellsymphony-host-adapter-shutdown-default-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    adapter.store_dir = temp_dir.clone();
    let payload = serde_json::json!({ "runtimeConfig": { "parts": [{ "name": "life" }] } });
    adapter
        .handle_platform_effect(&RuntimePlatformEffect::StoreSaveDefault {
            payload: payload.clone(),
            mode: Some("deferred".into()),
        })
        .unwrap();

    adapter.flush_pending_default_save_now().unwrap();

    let saved = std::fs::read_to_string(temp_dir.join("default.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
        payload
    );
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
