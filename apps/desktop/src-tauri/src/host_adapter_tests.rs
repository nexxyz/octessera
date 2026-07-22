use super::{DesktopHostAudioState, DesktopPlaybackHostAdapter};
use crate::audio_prep_service::{spawn_desktop_audio_control, DesktopAudioPrepState};
use crate::types::QueuedAudioEvent;
use playback_runtime::{RuntimePlatformEffect, RuntimePlatformRequest};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

fn temp_store_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "octessera-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn test_adapter() -> (DesktopPlaybackHostAdapter, mpsc::Receiver<QueuedAudioEvent>) {
    let (tx, rx) = mpsc::channel();
    let (platform_service_tx, _) = mpsc::channel();
    let synth_slots = Arc::new(Mutex::new(
        [true; realtime_engine::synth::INSTRUMENT_SLOT_COUNT],
    ));
    let sample_cache = Arc::new(Mutex::new(HashMap::new()));
    let sample_bank_signature = Arc::new(Mutex::new(String::new()));
    let (audio_control, _audio_prep_result_rx) = spawn_desktop_audio_control(
        tx.clone(),
        DesktopAudioPrepState {
            config_revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            synth_slots: synth_slots.clone(),
            sample_cache: sample_cache.clone(),
            sample_bank_signature: sample_bank_signature.clone(),
        },
    );
    let adapter = DesktopPlaybackHostAdapter {
        audio: DesktopHostAudioState {
            trigger_tx: tx,
            audio_control,
            sample_cache,
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

fn platform_request(effect: RuntimePlatformEffect) -> RuntimePlatformRequest {
    RuntimePlatformRequest::new(effect, "test-request".into(), None)
}

#[path = "host_adapter_audio_tests.rs"]
mod audio;
#[path = "host_adapter_platform_tests.rs"]
mod platform;
#[path = "host_adapter_store_tests.rs"]
mod store;
