use crate::audio_config::{
    decode_sample_file, normalize_config, sample_bank_signature, sample_banks, synth_payload,
    synth_slots, SampleBankError,
};
use crate::samples::resolve_sample_file;
use crate::types::QueuedAudioEvent;
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use serde_json::Value;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;

use super::DesktopAudioPrepState;

pub(super) struct PreparedAudioConfig {
    event: QueuedAudioEvent,
    synth_slots: [bool; INSTRUMENT_SLOT_COUNT],
    sample_signature: Option<String>,
}

pub(super) enum AudioPrepError {
    Superseded,
    InvalidConfig(String),
    Sample(SampleBankError),
    Failed(String),
}

pub(super) fn prepare_full_audio_config(
    revision: u64,
    request_id: Option<String>,
    config: Value,
    state: &DesktopAudioPrepState,
) -> Result<PreparedAudioConfig, AudioPrepError> {
    ensure_current_audio_revision(state, revision)?;
    let config = normalize_config(&config).map_err(AudioPrepError::InvalidConfig)?;
    let next_slots = synth_slots(&config);
    let next_sample_signature = sample_bank_signature(&config);
    let should_update_sample_banks = {
        let current = state
            .sample_bank_signature
            .lock()
            .map_err(|_| AudioPrepError::Failed("sample bank signature lock failed".into()))?;
        *current != next_sample_signature
    };
    let next_sample_banks = if should_update_sample_banks {
        Some(
            sample_banks(&config, resolve_sample_file, |path| {
                if let Ok(cache) = state.sample_cache.lock() {
                    if let Some(buffer) = cache.get(path) {
                        return Some(buffer.clone());
                    }
                } else {
                    return None;
                }
                let buffer = decode_sample_file(path)?;
                if let Ok(mut cache) = state.sample_cache.lock() {
                    cache.insert(path.to_string(), buffer.clone());
                }
                Some(buffer)
            })
            .map_err(AudioPrepError::Sample)?,
        )
    } else {
        None
    };
    ensure_current_audio_revision(state, revision)?;
    Ok(PreparedAudioConfig {
        event: QueuedAudioEvent::SetAudioConfig {
            revision,
            request_id,
            instruments: synth_payload(&config),
            sample_banks: next_sample_banks,
            voice_stealing_mode: config.voice_stealing_mode,
        },
        synth_slots: next_slots,
        sample_signature: should_update_sample_banks.then_some(next_sample_signature),
    })
}

pub(super) fn prepare_sample_preview(
    instrument_slot: usize,
    path: &str,
    velocity: u8,
    state: &DesktopAudioPrepState,
) -> Result<QueuedAudioEvent, AudioPrepError> {
    let full_path = resolve_sample_file(path)
        .ok_or_else(|| AudioPrepError::Sample(SampleBankError::Unresolved(path.into())))?;
    let buffer = if let Ok(cache) = state.sample_cache.lock() {
        cache.get(&full_path).cloned()
    } else {
        return Err(AudioPrepError::Failed("sample cache lock failed".into()));
    };
    let buffer = match buffer {
        Some(buffer) => buffer,
        None => {
            let buffer = decode_sample_file(&full_path)
                .ok_or_else(|| AudioPrepError::Sample(SampleBankError::Undecodable(path.into())))?;
            let mut cache = state
                .sample_cache
                .lock()
                .map_err(|_| AudioPrepError::Failed("sample cache lock failed".into()))?;
            cache.insert(full_path, buffer.clone());
            buffer
        }
    };
    Ok(QueuedAudioEvent::PreviewSample {
        instrument_slot: instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1) as u8,
        buffer,
        velocity,
    })
}

fn ensure_current_audio_revision(
    state: &DesktopAudioPrepState,
    revision: u64,
) -> Result<(), AudioPrepError> {
    (state.config_revision.load(Ordering::SeqCst) == revision)
        .then_some(())
        .ok_or(AudioPrepError::Superseded)
}

pub(super) fn apply_prepared_audio_config(
    prepared: PreparedAudioConfig,
    revision: u64,
    trigger_tx: &Sender<QueuedAudioEvent>,
    state: &DesktopAudioPrepState,
) -> Result<(), AudioPrepError> {
    ensure_current_audio_revision(state, revision)?;
    if state.synth_slots.lock().is_err() {
        return Err(AudioPrepError::Failed(
            "synth slot state lock failed".into(),
        ));
    }
    if prepared.sample_signature.is_some() && state.sample_bank_signature.lock().is_err() {
        return Err(AudioPrepError::Failed(
            "sample bank signature lock failed".into(),
        ));
    }
    trigger_tx.send(prepared.event).map_err(|error| {
        AudioPrepError::Failed(format!("audio engine queue send failed: {error}"))
    })?;
    if let Ok(mut slots) = state.synth_slots.lock() {
        *slots = prepared.synth_slots;
    } else {
        return Err(AudioPrepError::Failed(
            "synth slot state lock failed".into(),
        ));
    }
    if let Some(signature) = prepared.sample_signature {
        if let Ok(mut current) = state.sample_bank_signature.lock() {
            *current = signature;
        } else {
            return Err(AudioPrepError::Failed(
                "sample bank signature lock failed".into(),
            ));
        }
    }
    Ok(())
}
