use crate::runtime_worker::{request_worker_audio_command, request_worker_dispatch};
use crate::types::{
    encode_runtime_responses, AudioCommandPayload, MomentaryFxTargetPayload, RuntimeMessagesPayload,
};
use playback_runtime::RuntimeAudioCommand;
use realtime_engine::synth::SAMPLE_SLOTS_PER_INSTRUMENT;
use serde_json::Value;

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
pub(crate) fn runtime_dispatch(
    message: Value,
    state: tauri::State<crate::AppState>,
) -> Result<Vec<Value>, String> {
    let host_message = serde_json::from_value::<playback_runtime::HostMessage>(message)
        .map_err(|e| format!("invalid runtime host message: {e}"))?;
    encode_runtime_responses(request_worker_dispatch(&state, host_message)?)
}
