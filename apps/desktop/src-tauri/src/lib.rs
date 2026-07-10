mod audio_config;
mod audio_prep_service;
mod audio_thread;
mod commands;
mod desktop_platform_service;
mod host_adapter;
mod midi;
mod persistence;
mod runtime_worker;
mod samples;
mod types;

pub(crate) use types::SampleSlotConfig;

use audio_prep_service::{spawn_desktop_audio_control, DesktopAudioPrepState};
use audio_thread::{spawn_audio_engine_thread, spawn_load_listener};
use desktop_platform_service::spawn_desktop_platform_service;
use host_adapter::{DesktopHostAudioState, DesktopPlaybackHostAdapter};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use runtime_worker::{RuntimeWorker, WorkerCommand};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub(crate) struct AppState {
    worker_tx: mpsc::Sender<crate::runtime_worker::WorkerCommand>,
    runtime_outbox: Arc<Mutex<Vec<crate::types::RuntimeMessagesPayload>>>,
    audio_error: Arc<Mutex<Option<String>>>,
}

const BUNDLED_DEFAULT_CONFIG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../config/generated/desktop/default.json"
));

fn ensure_store_dir(app: &tauri::App) -> PathBuf {
    if let Some(dir) = std::env::var_os("OCTESSERA_DESKTOP_STORE_DIR").map(PathBuf::from) {
        return ensure_store_dir_at(dir);
    }
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| fallback_store_dir());
    ensure_store_dir_at(dir)
}

fn ensure_store_dir_at(dir: PathBuf) -> PathBuf {
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("presets"));
    let default_path = dir.join("default.json");
    if !default_path.is_file() {
        let _ = serde_json::from_str(BUNDLED_DEFAULT_CONFIG)
            .map_err(|_| ())
            .and_then(|payload| {
                persistence::atomic_write_json(&default_path, &payload).map_err(|_| ())
            });
    }
    dir
}

fn fallback_store_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("config")))
        .unwrap_or_else(|| PathBuf::from("config"))
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
    spawn_audio_engine_thread(trigger_rx, load_tx, audio_error.clone(), no_audio);

    tauri::Builder::default()
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let store_dir = ensure_store_dir(app);
            let platform_service = spawn_desktop_platform_service();
            let audio_control = spawn_desktop_audio_control(
                trigger_tx.clone(),
                DesktopAudioPrepState {
                    synth_slots: synth_slots.clone(),
                    sample_cache: sample_cache.clone(),
                    sample_bank_signature: sample_bank_signature.clone(),
                },
            );
            let (native_midi_tx, native_midi_rx) = mpsc::channel::<Vec<u8>>();
            let worker_tx = RuntimeWorker::spawn(
                app_handle.clone(),
                audio_error.clone(),
                runtime_outbox.clone(),
                DesktopPlaybackHostAdapter::new(
                    DesktopHostAudioState {
                        trigger_tx: trigger_tx.clone(),
                        audio_control,
                        sample_cache: sample_cache.clone(),
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
            spawn_load_listener(load_rx, app_handle);

            app.manage(AppState {
                worker_tx,
                runtime_outbox,
                audio_error,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::audio_command,
            commands::runtime_dispatch,
            commands::runtime_sync_config,
            commands::runtime_drain_messages,
            samples::sample_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_dir_seed_writes_bundled_default_once() {
        let dir = std::env::temp_dir().join(format!(
            "octessera-desktop-store-seed-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let seeded = ensure_store_dir_at(dir.clone());
        let default_path = seeded.join("default.json");
        let default_payload: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&default_path).unwrap()).unwrap();
        assert_eq!(
            default_payload["runtimeConfig"]["parts"][3]["autoName"],
            true
        );
        assert_eq!(default_payload["runtimeConfig"]["displayBrightness"], 100);
        assert_eq!(default_payload["runtimeConfig"]["gridBrightness"], 100);
        assert_eq!(default_payload["runtimeConfig"]["buttonBrightness"], 100);

        std::fs::write(&default_path, "{\"kept\":true}").unwrap();
        ensure_store_dir_at(dir.clone());
        assert_eq!(
            std::fs::read_to_string(&default_path).unwrap(),
            "{\"kept\":true}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
