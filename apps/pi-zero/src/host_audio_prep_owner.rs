use super::prep_results::{
    audio_queue_failure, index_u8, owner_failure, owner_sample_failure, prep_error_message,
    send_audio_prep_result, AudioPrepError,
};
use crate::audio::AudioService;
use crate::audio_config_parse::{
    parse_normalized_instrument_slot_config, sample_bank_for_instrument,
};
use playback_runtime::HostMessage;
use realtime_engine::synth::{
    prepare_fx_bus_slot, prepare_global_fx_slot, prepare_instrument_slot_config, validate_fx_type,
    DEFAULT_AUDIO_SAMPLE_RATE,
};
use rodio_engine_source::EngineEvent;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

pub(super) enum OwnerRequest {
    Instrument {
        sequence: u64,
        instrument_slot: usize,
        generation: u64,
        config: serde_json::Value,
        samples_dir: PathBuf,
    },
    FxBus {
        sequence: u64,
        bus_index: usize,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: BTreeMap<String, serde_json::Value>,
    },
    GlobalFx {
        sequence: u64,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: BTreeMap<String, serde_json::Value>,
    },
}

struct OwnerPrepared {
    event: EngineEvent,
}

pub(super) fn process_owner_request(
    audio: &AudioService,
    request: OwnerRequest,
    result_tx: &Sender<HostMessage>,
) {
    let sequence = match &request {
        OwnerRequest::Instrument { sequence, .. }
        | OwnerRequest::FxBus { sequence, .. }
        | OwnerRequest::GlobalFx { sequence, .. } => *sequence,
    };
    if audio
        .latest_full_sequence
        .load(std::sync::atomic::Ordering::Acquire)
        > sequence
    {
        return;
    }
    let instrument_commit = match &request {
        OwnerRequest::Instrument {
            sequence: _,
            instrument_slot,
            generation,
            ..
        } => Some((*instrument_slot, *generation)),
        OwnerRequest::FxBus { .. } | OwnerRequest::GlobalFx { .. } => None,
    };
    let result = match request {
        OwnerRequest::Instrument {
            sequence: _,
            instrument_slot,
            generation,
            config,
            samples_dir,
        } => prepare_instrument(audio, instrument_slot, generation, config, samples_dir),
        OwnerRequest::FxBus {
            sequence: _,
            bus_index,
            slot_index,
            generation,
            fx_type,
            params,
        } => validate_fx_type(&fx_type)
            .map_err(|error| AudioPrepError::InvalidConfig(error.to_string()))
            .map(|_| OwnerPrepared {
                event: EngineEvent::SetPreparedFxBusSlot {
                    bus_index: index_u8(bus_index),
                    slot_index: index_u8(slot_index),
                    generation,
                    config: prepare_fx_bus_slot(fx_type, params, DEFAULT_AUDIO_SAMPLE_RATE),
                },
            }),
        OwnerRequest::GlobalFx {
            sequence: _,
            slot_index,
            generation,
            fx_type,
            params,
        } => validate_fx_type(&fx_type)
            .map_err(|error| AudioPrepError::InvalidConfig(error.to_string()))
            .map(|_| OwnerPrepared {
                event: EngineEvent::SetPreparedGlobalFxSlot {
                    slot_index: index_u8(slot_index),
                    generation,
                    config: prepare_global_fx_slot(fx_type, params),
                },
            }),
    };
    match result {
        Ok(prepared) => {
            if audio
                .latest_full_sequence
                .load(std::sync::atomic::Ordering::Acquire)
                > sequence
            {
                return;
            }
            let sample_owner = matches!(
                &prepared.event,
                EngineEvent::SetPreparedInstrumentOwner {
                    sample_bank: Some(_),
                    ..
                }
            );
            if let Err(error) = audio.broadcast(prepared.event) {
                send_audio_prep_result(result_tx, audio_queue_failure(error));
            } else if let Some((instrument_slot, generation)) = instrument_commit {
                let commit_result =
                    commit_instrument_generation(audio, instrument_slot, generation, sample_owner);
                if let Err(error) = commit_result {
                    send_audio_prep_result(result_tx, audio_queue_failure(error));
                }
            }
        }
        Err(AudioPrepError::Sample(error)) => send_audio_prep_result(
            result_tx,
            owner_sample_failure(error.code(), error.message()),
        ),
        Err(error) => send_audio_prep_result(result_tx, owner_failure(prep_error_message(error))),
    }
}

fn prepare_instrument(
    audio: &AudioService,
    instrument_slot: usize,
    generation: u64,
    config: serde_json::Value,
    samples_dir: PathBuf,
) -> Result<OwnerPrepared, AudioPrepError> {
    let normalized =
        parse_normalized_instrument_slot_config(&config).map_err(AudioPrepError::InvalidConfig)?;
    let sample_bank = sample_bank_for_instrument(&normalized, &samples_dir, audio)
        .map_err(AudioPrepError::Sample)?;
    Ok(OwnerPrepared {
        event: EngineEvent::SetPreparedInstrumentOwner {
            instrument_slot: index_u8(instrument_slot),
            generation,
            config: prepare_instrument_slot_config(normalized.slot),
            sample_bank,
        },
    })
}

fn commit_instrument_generation(
    audio: &AudioService,
    instrument_slot: usize,
    generation: u64,
    sample_owner: bool,
) -> Result<(), String> {
    if sample_owner {
        audio
            .sample_bank_signature
            .lock()
            .map_err(|_| "sample bank signature lock failed".to_string())?
            .clear();
    }
    let mut generations = audio
        .generations
        .lock()
        .map_err(|_| "audio generation state lock failed".to_string())?;
    if let Some(current) = generations.instrument.get_mut(instrument_slot) {
        *current = (*current).max(generation);
    }
    if sample_owner {
        if let Some(current) = generations.sample.get_mut(instrument_slot) {
            *current = (*current).max(generation);
        }
    }
    Ok(())
}

pub(super) fn process_instrument_slot(
    audio: &AudioService,
    sequence: u64,
    instrument_slot: usize,
    generation: u64,
    config: serde_json::Value,
    samples_dir: PathBuf,
    result_tx: &Sender<HostMessage>,
) {
    process_owner_request(
        audio,
        OwnerRequest::Instrument {
            sequence,
            instrument_slot,
            generation,
            config,
            samples_dir,
        },
        result_tx,
    );
}
