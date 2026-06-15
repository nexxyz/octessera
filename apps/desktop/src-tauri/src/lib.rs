mod audio_config;
mod audio_thread;
mod commands;
mod host_adapter;
mod midi;
mod runtime_worker;
mod samples;
mod types;

pub(crate) use types::SampleSlotConfig;

use audio_thread::{spawn_audio_engine_thread, spawn_load_listener};
use host_adapter::DesktopPlaybackHostAdapter;
use midir::MidiInputConnection;
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use runtime_worker::{ensure_store_dir, RuntimeWorker};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

pub(crate) struct AppState {
    pub(crate) trigger_tx: mpsc::Sender<crate::types::QueuedAudioEvent>,
    worker_tx: mpsc::Sender<crate::runtime_worker::WorkerCommand>,
    runtime_outbox: Arc<Mutex<Vec<crate::types::RuntimeMessagesPayload>>>,
    synth_slots: Arc<Mutex<[bool; INSTRUMENT_SLOT_COUNT]>>,
    sample_cache: Arc<Mutex<HashMap<String, realtime_engine::synth::SampleBuffer>>>,
    sample_bank_signature: Arc<Mutex<String>>,
    pub(crate) midi_out: Arc<Mutex<Option<midir::MidiOutputConnection>>>,
    pub(crate) midi_in: Arc<Mutex<Option<MidiInputConnection<()>>>>,
    audio_error: Arc<Mutex<Option<String>>>,
    store_dir: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let no_audio = std::env::args().any(|arg| arg == "--no-audio");

    let audio_error = Arc::new(Mutex::new(None::<String>));
    let (trigger_tx, trigger_rx) = mpsc::channel::<crate::types::QueuedAudioEvent>();
    let (load_tx, load_rx) = mpsc::channel::<realtime_engine::synth::AudioLoadStatus>();
    let synth_slots = Arc::new(Mutex::new([true; INSTRUMENT_SLOT_COUNT]));
    let sample_cache = Arc::new(Mutex::new(HashMap::new()));
    let sample_bank_signature = Arc::new(Mutex::new(String::new()));
    let midi_out = Arc::new(Mutex::new(None));
    let midi_in = Arc::new(Mutex::new(None));
    let runtime_outbox = Arc::new(Mutex::new(Vec::new()));
    let store_dir = ensure_store_dir();

    spawn_audio_engine_thread(trigger_rx, load_tx, audio_error.clone(), no_audio);

    tauri::Builder::default()
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let midi_in_app_handle = app_handle.clone();
            let worker_tx = RuntimeWorker::spawn(
                app_handle.clone(),
                audio_error.clone(),
                runtime_outbox.clone(),
                DesktopPlaybackHostAdapter::new(
                    trigger_tx.clone(),
                    sample_cache.clone(),
                    midi_out.clone(),
                    midi_in.clone(),
                    Arc::new(move |bytes| {
                        let _ = midi_in_app_handle.emit("midi_in", midi::MidiInMessage { bytes });
                    }),
                    store_dir.clone(),
                ),
            );
            spawn_load_listener(load_rx, app_handle);

            app.manage(AppState {
                trigger_tx,
                worker_tx,
                runtime_outbox,
                synth_slots,
                sample_cache,
                sample_bank_signature,
                midi_out,
                midi_in,
                audio_error,
                store_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::audio_set_instruments,
            commands::audio_set_runtime_policy,
            commands::audio_command,
            commands::core_runner_dispatch,
            commands::runtime_dispatch,
            commands::runtime_handle_midi_realtime,
            commands::runtime_advance,
            commands::runtime_sync_config,
            commands::runtime_drain_messages,
            commands::core_runner_reset,
            commands::store_save_default,
            commands::store_load_default,
            midi::midi_list_outputs,
            midi::midi_list_inputs,
            midi::midi_select_output,
            midi::midi_select_input,
            midi::midi_send,
            samples::sample_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
