use crate::audio_config::{
    build_audio_slot_configs, decode_sample_file, parse_voice_stealing_mode, sample_bank_signature,
    sample_banks, synth_payload, AudioInstrumentsConfig, AudioRuntimePolicyConfig,
};
use crate::runtime_worker::{
    request_worker_audio_command, request_worker_dispatch, request_worker_midi_realtime,
    request_worker_reset, WorkerCommand,
};
use crate::samples::resolve_sample_file;
use crate::types::{
    append_audio_error_values, encode_runtime_responses, AudioCommandPayload,
    MomentaryFxTargetPayload, QueuedAudioEvent, RuntimeMessagesPayload,
};
use playback_runtime::RuntimeAudioCommand;
use realtime_engine::synth::SAMPLE_SLOTS_PER_INSTRUMENT;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub(crate) struct PlaybackRuntimeConfigPayload {
    bpm: f64,
    #[serde(rename = "syncSource")]
    sync_source: playback_runtime::SyncSource,
    #[serde(rename = "midiClockOutEnabled")]
    midi_clock_out_enabled: bool,
    #[serde(rename = "midiOutEnabled")]
    midi_out_enabled: bool,
}

#[tauri::command]
pub(crate) fn runtime_drain_messages(
    state: tauri::State<crate::AppState>,
) -> Result<Vec<RuntimeMessagesPayload>, String> {
    let mut guard = state
        .runtime_outbox
        .lock()
        .map_err(|_| "runtime outbox mutex poisoned".to_string())?;
    Ok(std::mem::take(&mut *guard))
}

#[tauri::command]
pub(crate) fn audio_set_instruments(
    config: AudioInstrumentsConfig,
    state: tauri::State<crate::AppState>,
) -> Result<(), String> {
    let (next_slots, _) = build_audio_slot_configs(&config.instruments);
    let next_sample_signature = sample_bank_signature(&config);
    let should_update_sample_banks = {
        let mut current = state
            .sample_bank_signature
            .lock()
            .map_err(|_| "sample bank signature lock failed".to_string())?;
        if *current == next_sample_signature {
            false
        } else {
            *current = next_sample_signature;
            true
        }
    };
    let next_sample_banks = if should_update_sample_banks {
        let mut cache = state
            .sample_cache
            .lock()
            .map_err(|_| "sample cache lock failed".to_string())?;
        Some(sample_banks(&config, resolve_sample_file, |path| {
            if let Some(buffer) = cache.get(path) {
                return Some(buffer.clone());
            }
            let buffer = decode_sample_file(path)?;
            cache.insert(path.to_string(), buffer.clone());
            Some(buffer)
        }))
    } else {
        None
    };
    if let Ok(mut slots) = state.synth_slots.lock() {
        *slots = next_slots;
    }
    let synth_payload = synth_payload(&config);
    state
        .trigger_tx
        .send(QueuedAudioEvent::SetInstruments(synth_payload))
        .map_err(|e| format!("audio queue send failed: {e}"))?;
    if let Some(next_sample_banks) = next_sample_banks {
        state
            .trigger_tx
            .send(QueuedAudioEvent::SetSampleBanks(next_sample_banks))
            .map_err(|e| format!("audio queue send failed: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn audio_set_runtime_policy(
    policy: AudioRuntimePolicyConfig,
    state: tauri::State<crate::AppState>,
) -> Result<(), String> {
    let mode = parse_voice_stealing_mode(&policy.voice_stealing_mode);
    state
        .trigger_tx
        .send(QueuedAudioEvent::SetVoiceStealingMode(mode))
        .map_err(|e| format!("audio queue send failed: {e}"))
}

#[tauri::command]
pub(crate) fn audio_command(
    command: AudioCommandPayload,
    state: tauri::State<crate::AppState>,
) -> Result<(), String> {
    let runtime_command = match command {
        AudioCommandPayload::MomentaryFxStart {
            id,
            fx_type,
            params,
            target,
        } => RuntimeAudioCommand::MomentaryFxStart {
            id,
            fx_type,
            params,
            target: match target {
                MomentaryFxTargetPayload::Global => {
                    playback_runtime::RuntimeMomentaryFxTarget::Global
                }
                MomentaryFxTargetPayload::FxBus { index } => {
                    playback_runtime::RuntimeMomentaryFxTarget::FxBus { index }
                }
                MomentaryFxTargetPayload::Instrument { index } => {
                    playback_runtime::RuntimeMomentaryFxTarget::Instrument { index }
                }
            },
        },
        AudioCommandPayload::MomentaryFxUpdate { id, params } => {
            RuntimeAudioCommand::MomentaryFxUpdate { id, params }
        }
        AudioCommandPayload::MomentaryFxStop { id } => RuntimeAudioCommand::MomentaryFxStop { id },
        AudioCommandPayload::SamplePreview {
            instrument_slot,
            sample_slot,
            path,
            velocity,
        } => RuntimeAudioCommand::SamplePreview {
            instrument_slot,
            sample_slot: sample_slot.min(SAMPLE_SLOTS_PER_INSTRUMENT - 1),
            path,
            velocity,
        },
    };
    request_worker_audio_command(&state, runtime_command)
}

#[tauri::command]
pub(crate) fn runtime_sync_config(
    config: PlaybackRuntimeConfigPayload,
    state: tauri::State<crate::AppState>,
) -> Result<(), String> {
    state
        .worker_tx
        .send(WorkerCommand::SyncConfig(playback_runtime::RuntimeConfig {
            bpm: config.bpm,
            sync_source: config.sync_source,
            midi_clock_out_enabled: config.midi_clock_out_enabled,
            midi_out_enabled: config.midi_out_enabled,
        }))
        .map_err(|e| format!("runtime worker unavailable: {e}"))
}

#[tauri::command]
pub(crate) fn core_runner_dispatch(
    message: Value,
    state: tauri::State<crate::AppState>,
) -> Result<Vec<Value>, String> {
    let host_message = serde_json::from_value::<playback_runtime::HostMessage>(message)
        .map_err(|e| format!("invalid core runner host message: {e}"))?;
    Ok(append_audio_error_values(
        encode_runtime_responses(request_worker_dispatch(&state, host_message)?)?,
        &state.audio_error,
    ))
}

#[tauri::command]
pub(crate) fn runtime_dispatch(
    message: Value,
    state: tauri::State<crate::AppState>,
) -> Result<Vec<Value>, String> {
    let host_message = serde_json::from_value::<playback_runtime::HostMessage>(message)
        .map_err(|e| format!("invalid runtime host message: {e}"))?;
    Ok(append_audio_error_values(
        encode_runtime_responses(request_worker_dispatch(&state, host_message)?)?,
        &state.audio_error,
    ))
}

#[tauri::command]
pub(crate) fn runtime_handle_midi_realtime(
    bytes: Vec<u8>,
    state: tauri::State<crate::AppState>,
) -> Result<Vec<Value>, String> {
    Ok(append_audio_error_values(
        encode_runtime_responses(request_worker_midi_realtime(&state, bytes)?)?,
        &state.audio_error,
    ))
}

#[tauri::command]
pub(crate) fn runtime_advance(
    _elapsed_ms: u64,
    _state: tauri::State<crate::AppState>,
) -> Result<Vec<Value>, String> {
    Ok(Vec::new())
}

#[tauri::command]
pub(crate) fn core_runner_reset(state: tauri::State<crate::AppState>) -> Result<(), String> {
    request_worker_reset(&state)
}

#[tauri::command]
pub(crate) fn store_save_default(
    payload: Value,
    state: tauri::State<crate::AppState>,
) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    std::fs::write(state.store_dir.join("default.json"), content).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn store_load_default(
    state: tauri::State<crate::AppState>,
) -> Result<Option<Value>, String> {
    let path = state.store_dir.join("default.json");
    if path.is_file() {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}
