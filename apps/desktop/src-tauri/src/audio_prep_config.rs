use crate::audio_config::{
    normalize_config, parse_instrument_slot_config, sample_bank_for_slot_config,
    sample_bank_signature, sample_banks, synth_payload, synth_slots, SampleBankError,
};
use crate::sample_decode_cache::SampleDecodeCacheError;
use crate::samples::resolve_sample_file;
use realtime_engine::synth::{
    prepare_audio_config, prepare_fx_bus_slot, prepare_global_fx_slot,
    prepare_instrument_slot_config, DEFAULT_AUDIO_SAMPLE_RATE, INSTRUMENT_SLOT_COUNT,
};
use rodio_engine_source::EngineEvent;
use serde_json::Value;
use std::sync::atomic::Ordering;

use super::DesktopAudioPrepState;

pub(super) enum AudioPrepError {
    Superseded,
    InvalidConfig(String),
    Sample(SampleBankError),
    Failed(String),
}

pub(super) struct PreparedAudioConfig {
    pub(super) event: EngineEvent,
    pub(super) synth_slots: [bool; INSTRUMENT_SLOT_COUNT],
    pub(super) sample_signature: Option<String>,
}

pub(super) fn prepare_full_audio_config(
    revision: u64,
    generation: u64,
    _request_id: Option<String>,
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
                match state.sample_decode_cache.load(path) {
                    Ok(buffer) => buffer,
                    Err(SampleDecodeCacheError::LookupLock) => None,
                    Err(SampleDecodeCacheError::InsertionLock(buffer)) => Some(buffer),
                }
            })
            .map_err(AudioPrepError::Sample)?,
        )
    } else {
        None
    };
    ensure_current_audio_revision(state, revision)?;
    Ok(PreparedAudioConfig {
        event: EngineEvent::SetPreparedAudioConfig {
            generation,
            config: prepare_audio_config(
                synth_payload(&config),
                next_sample_banks,
                config.voice_stealing_mode,
                DEFAULT_AUDIO_SAMPLE_RATE,
            ),
        },
        synth_slots: next_slots,
        sample_signature: should_update_sample_banks.then_some(next_sample_signature),
    })
}

pub(super) fn commit_full_audio_config(
    prepared: &PreparedAudioConfig,
    state: &DesktopAudioPrepState,
    generation: u64,
) -> Result<(), AudioPrepError> {
    let mut slots = state
        .synth_slots
        .lock()
        .map_err(|_| AudioPrepError::Failed("synth slot state lock failed".into()))?;
    *slots = prepared.synth_slots;
    drop(slots);
    if let Some(signature) = &prepared.sample_signature {
        let mut current = state
            .sample_bank_signature
            .lock()
            .map_err(|_| AudioPrepError::Failed("sample bank signature lock failed".into()))?;
        *current = signature.clone();
    }
    let mut generations = state
        .generations
        .lock()
        .map_err(|_| AudioPrepError::Failed("audio generation state lock failed".into()))?;
    let full = generations.full.max(generation);
    generations.full = full;
    generations.instrument.fill(full);
    generations.sample.fill(full);
    generations.bus_mixer.fill(full);
    generations
        .fx_bus
        .fill([full; realtime_engine::synth::BUS_SLOTS_PER_BUS]);
    generations.global_fx.fill(full);
    Ok(())
}

pub(super) fn prepare_instrument_slot_event(
    instrument_slot: usize,
    generation: u64,
    config: Value,
    state: &DesktopAudioPrepState,
) -> Result<EngineEvent, AudioPrepError> {
    let parsed = parse_instrument_slot_config(&config).map_err(AudioPrepError::InvalidConfig)?;
    let sample_bank = sample_bank_for_slot_config(&config, resolve_sample_file, |path| match state
        .sample_decode_cache
        .load(path)
    {
        Ok(buffer) => buffer,
        Err(SampleDecodeCacheError::LookupLock) => None,
        Err(SampleDecodeCacheError::InsertionLock(buffer)) => Some(buffer),
    })
    .map_err(AudioPrepError::Sample)?;
    Ok(EngineEvent::SetPreparedInstrumentOwner {
        instrument_slot: instrument_slot as u8,
        generation,
        config: prepare_instrument_slot_config(parsed),
        sample_bank,
    })
}

pub(super) fn prepare_fx_bus_slot_event(
    bus_index: usize,
    slot_index: usize,
    generation: u64,
    fx_type: String,
    params: std::collections::BTreeMap<String, Value>,
) -> EngineEvent {
    EngineEvent::SetPreparedFxBusSlot {
        bus_index: bus_index as u8,
        slot_index: slot_index as u8,
        generation,
        config: prepare_fx_bus_slot(fx_type, params, DEFAULT_AUDIO_SAMPLE_RATE),
    }
}

pub(super) fn prepare_global_fx_slot_event(
    slot_index: usize,
    generation: u64,
    fx_type: String,
    params: std::collections::BTreeMap<String, Value>,
) -> EngineEvent {
    EngineEvent::SetPreparedGlobalFxSlot {
        slot_index: slot_index as u8,
        generation,
        config: prepare_global_fx_slot(fx_type, params),
    }
}

pub(super) fn prepare_sample_preview(
    instrument_slot: usize,
    generation: u64,
    path: &str,
    velocity: u8,
    state: &DesktopAudioPrepState,
) -> Result<EngineEvent, AudioPrepError> {
    let full_path = resolve_sample_file(path)
        .ok_or_else(|| AudioPrepError::Sample(SampleBankError::Unresolved(path.into())))?;
    let buffer = match state.sample_decode_cache.load(&full_path) {
        Ok(Some(buffer)) => buffer,
        Ok(None) => {
            return Err(AudioPrepError::Sample(SampleBankError::Undecodable(
                path.into(),
            )))
        }
        Err(_) => return Err(AudioPrepError::Failed("sample cache lock failed".into())),
    };
    Ok(EngineEvent::PreviewSample {
        instrument_slot: instrument_slot as u8,
        generation,
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
