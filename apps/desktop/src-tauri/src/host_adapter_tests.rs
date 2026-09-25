use super::{DesktopHostAudioState, DesktopPlaybackHostAdapter};
use crate::audio_prep_service::{
    spawn_desktop_audio_control, AudioGenerationState, DesktopAudioPrepState,
};
use crate::recording::DesktopRecording;
use crate::sample_decode_cache::SampleDecodeCache;
use playback_runtime::{RuntimePlatformEffect, RuntimePlatformRequest};
use rodio_engine_source::{event_queue, EngineEvent, EngineEventReceiver};
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

fn test_adapter() -> (DesktopPlaybackHostAdapter, EngineEventReceiver) {
    test_adapter_with_recording_dir(
        std::env::temp_dir().join(format!("octessera-recording-test-{}", std::process::id())),
    )
}

fn test_adapter_with_recording_dir(
    recording_dir: PathBuf,
) -> (DesktopPlaybackHostAdapter, EngineEventReceiver) {
    let (engine_tx, engine_rx) = event_queue();
    let (platform_service_tx, _) = mpsc::sync_channel(32);
    let synth_slots = Arc::new(Mutex::new(
        [true; realtime_engine::synth::INSTRUMENT_SLOT_COUNT],
    ));
    let sample_decode_cache = SampleDecodeCache::new();
    let sample_bank_signature = Arc::new(Mutex::new(String::new()));
    let generations = Arc::new(Mutex::new(AudioGenerationState::default()));
    let (audio_control, _audio_prep_result_rx) = spawn_desktop_audio_control(
        engine_tx.clone(),
        DesktopAudioPrepState {
            config_revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            synth_slots: synth_slots.clone(),
            sample_decode_cache: sample_decode_cache.clone(),
            sample_bank_signature: sample_bank_signature.clone(),
            generations,
        },
    );
    let adapter = DesktopPlaybackHostAdapter {
        audio: DesktopHostAudioState {
            engine_tx,
            audio_control,
            recording: DesktopRecording::new(recording_dir),
        },
        midi_out: Arc::new(Mutex::new(None)),
        midi_in: Arc::new(Mutex::new(None)),
        midi_in_handler: Arc::new(|_| {}),
        store_dir: PathBuf::new(),
        pending_default_save: playback_runtime::DeferredDefaultSave::default(),
        platform_service_tx,
        selected_midi_output_id: None,
        selected_midi_input_id: None,
        shutdown_requested: false,
        momentary_fx_types: std::collections::HashMap::new(),
    };
    (adapter, engine_rx)
}

fn next_event(rx: &mut EngineEventReceiver) -> EngineEvent {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        match rx.try_recv() {
            Ok(event) => return event,
            Err(_) => {
                assert!(std::time::Instant::now() < deadline, "audio event timeout");
                std::thread::yield_now();
            }
        }
    }
}

fn platform_request(effect: RuntimePlatformEffect) -> RuntimePlatformRequest {
    RuntimePlatformRequest::new(effect, "test-request".into(), None)
}

#[path = "host_adapter_audio_tests.rs"]
mod audio;
#[path = "host_adapter_drum_tests.rs"]
mod drum;
#[path = "host_adapter_momentary_tests.rs"]
mod momentary;
#[path = "host_adapter_platform_tests.rs"]
mod platform;
#[path = "host_adapter_recording_tests.rs"]
mod recording;
#[path = "host_adapter_sampler_tests.rs"]
mod sampler;
#[path = "host_adapter_store_tests.rs"]
mod store;
