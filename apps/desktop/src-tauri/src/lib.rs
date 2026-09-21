mod audio_config;
mod audio_prep_service;
mod audio_thread;
mod commands;
mod desktop_platform_service;
mod host_adapter;
mod midi;
mod persistence;
mod recording;
mod runtime_worker;
mod sample_decode_cache;
mod samples;
mod startup_failure;
mod store_startup;
mod types;

use audio_prep_service::{
    spawn_desktop_audio_control, AudioGenerationState, DesktopAudioPrepState,
};
use audio_thread::{spawn_audio_engine_thread, spawn_load_listener};
use desktop_platform_service::spawn_desktop_platform_service;
use host_adapter::{DesktopHostAudioState, DesktopPlaybackHostAdapter};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use rodio_engine_source::event_queue;
use runtime_worker::{RuntimeWorker, WorkerCommand};
use sample_decode_cache::SampleDecodeCache;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub(crate) struct AppState {
    worker_tx: mpsc::Sender<crate::runtime_worker::WorkerCommand>,
    runtime_outbox: Arc<Mutex<Vec<crate::types::RuntimeMessagesPayload>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let no_audio = std::env::args().any(|arg| arg == "--no-audio");

    let (engine_tx, engine_rx) = event_queue();
    let (load_tx, load_rx) = rodio_engine_source::audio_load_status_channel();
    let (audio_failure_tx, audio_failure_rx) =
        mpsc::channel::<playback_runtime::RuntimeAdapterError>();
    let synth_slots = Arc::new(Mutex::new([true; INSTRUMENT_SLOT_COUNT]));
    let sample_decode_cache = SampleDecodeCache::new();
    let sample_bank_signature = Arc::new(Mutex::new(String::new()));
    let generations = Arc::new(Mutex::new(AudioGenerationState::default()));
    let config_revision = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let midi_out = Arc::new(Mutex::new(None));
    let midi_in = Arc::new(Mutex::new(None));
    let runtime_outbox = Arc::new(Mutex::new(Vec::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let startup_result = samples::initialize_samples_root(app)
                .and_then(|()| {
                    store_startup::ensure_store_dir(app).map_err(|error| error.to_string())
                })
                .and_then(|store_dir| {
                    recording::resolve_recordings_dir(app).and_then(|recording_dir| {
                        recording::resolve_screen_recordings_dir(app).map(|screen_recording_dir| {
                            (store_dir, recording_dir, screen_recording_dir)
                        })
                    })
                });
            let (store_dir, recording_dir, screen_recording_dir) =
                match startup_failure::decide_startup(startup_result, |title, message| {
                    startup_failure::present_native_startup_error(app, title, message);
                }) {
                    startup_failure::StartupDecision::Continue(paths) => paths,
                    startup_failure::StartupDecision::FailurePresented => return Ok(()),
                };
            let recording = recording::DesktopRecording::with_screen_directory(
                recording_dir,
                screen_recording_dir,
            );
            spawn_audio_engine_thread(
                engine_rx,
                load_tx,
                audio_failure_tx,
                no_audio,
                recording.tap_state(),
            );
            let platform_service = spawn_desktop_platform_service();
            let (audio_control, audio_prep_result_rx) = spawn_desktop_audio_control(
                engine_tx.clone(),
                DesktopAudioPrepState {
                    config_revision: config_revision.clone(),
                    synth_slots: synth_slots.clone(),
                    sample_decode_cache: sample_decode_cache.clone(),
                    sample_bank_signature: sample_bank_signature.clone(),
                    generations: generations.clone(),
                },
            );
            let (native_midi_tx, native_midi_rx) = mpsc::channel::<Vec<u8>>();
            let worker_tx = RuntimeWorker::spawn(
                app_handle.clone(),
                runtime_outbox.clone(),
                audio_failure_rx,
                audio_prep_result_rx,
                DesktopPlaybackHostAdapter::new(
                    DesktopHostAudioState {
                        engine_tx: engine_tx.clone(),
                        audio_control,
                        recording: recording.clone(),
                    },
                    midi_out.clone(),
                    midi_in.clone(),
                    Arc::new(move |bytes| {
                        let _ = native_midi_tx.send(bytes);
                    }),
                    store_dir.clone(),
                    platform_service.request_tx,
                ),
                platform_service.result_rx,
            );
            let midi_worker_tx = worker_tx.clone();
            std::thread::spawn(move || {
                while let Ok(bytes) = native_midi_rx.recv() {
                    if midi_worker_tx
                        .send(WorkerCommand::NativeMidiRealtime(bytes))
                        .is_err()
                    {
                        break;
                    }
                }
            });
            spawn_load_listener(load_rx, app_handle, worker_tx.clone());
            app.manage(AppState {
                worker_tx,
                runtime_outbox,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::audio_command,
            commands::runtime_dispatch,
            commands::runtime_drain_messages,
            samples::sample_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
