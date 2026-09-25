use playback_runtime::{
    RuntimeAdapterError, RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorFacts, RuntimeMomentaryFxTarget, RuntimeOperation,
};
use realtime_engine::synth::{
    normalize_audio_config, validate_fm_param_path, validate_fx_type, validate_momentary_fx_type,
    validate_pluck_param_path, validate_sample_bank_param_path, validate_synth_param_path,
    DrumParamId, MomentaryFxTarget, BUS_COUNT, BUS_SLOTS_PER_BUS, GLOBAL_FX_SLOT_COUNT,
    INSTRUMENT_SLOT_COUNT,
};
use std::path::Path;

pub(super) fn validate_audio_command(
    command: &RuntimeAudioCommand,
    _samples_dir: &Path,
) -> Result<(), RuntimeAdapterError> {
    match command {
        RuntimeAudioCommand::SetAudioConfig { config, .. } => {
            normalize_audio_config(config).map_err(invalid_audio_command)?;
        }
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot,
            config,
            ..
        } => {
            validate_instrument_slot(*instrument_slot)?;
            if !config.is_object() {
                return Err(invalid_audio_command(
                    "instrument slot must be an object".into(),
                ));
            }
        }
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot,
            path,
            value,
            ..
        } => {
            validate_instrument_slot(*instrument_slot)?;
            validate_synth_param_path(path).map_err(invalid_audio_command)?;
            ensure_finite(*value, "synth parameter")?;
        }
        RuntimeAudioCommand::SetFmParam {
            instrument_slot,
            path,
            value,
            ..
        } => {
            validate_instrument_slot(*instrument_slot)?;
            validate_fm_param_path(path).map_err(invalid_audio_command)?;
            ensure_finite(*value, "FM parameter")?;
        }
        RuntimeAudioCommand::SetPluckParam {
            instrument_slot,
            path,
            value,
            ..
        } => {
            validate_instrument_slot(*instrument_slot)?;
            validate_pluck_param_path(path).map_err(invalid_audio_command)?;
            ensure_finite(*value, "Plucked parameter")?;
        }
        RuntimeAudioCommand::SetDrumParam {
            instrument_slot,
            voice,
            path,
            value,
            ..
        } => {
            validate_instrument_slot(*instrument_slot)?;
            let param = DrumParamId::from_path(path).ok_or_else(|| {
                invalid_audio_command(format!("unsupported Drum parameter path `{path}`"))
            })?;
            if *voice >= 8 || (!param.is_voice_param() && *voice != 0) {
                return Err(invalid_audio_command(format!("invalid Drum voice {voice}")));
            }
            ensure_finite(*value, "Drum parameter")?;
        }
        RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot,
            path,
            value,
            ..
        } => {
            validate_instrument_slot(*instrument_slot)?;
            validate_sample_bank_param_path(path).map_err(invalid_audio_command)?;
            ensure_finite(*value, "sample parameter")?;
        }
        RuntimeAudioCommand::SetFxBusSlot {
            bus_index,
            slot_index,
            fx_type,
            params,
            ..
        } => {
            validate_fx_bus_slot(*bus_index, *slot_index)?;
            validate_fx_type(fx_type).map_err(invalid_audio_command)?;
            validate_params(params)?;
        }
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index,
            fx_type,
            params,
            ..
        } => {
            validate_global_fx_slot(*slot_index)?;
            validate_fx_type(fx_type).map_err(invalid_audio_command)?;
            validate_params(params)?;
        }
        RuntimeAudioCommand::MomentaryFxStart {
            fx_type,
            params,
            target,
            ..
        } => {
            validate_momentary_fx_type(fx_type).map_err(invalid_audio_command)?;
            validate_params(params)?;
            validate_momentary_target(target)?;
        }
        RuntimeAudioCommand::MomentaryFxUpdate { params, .. } => validate_params(params)?,
        RuntimeAudioCommand::SetMasterVolume { volume_pct, .. } => {
            ensure_finite(*volume_pct, "master volume")?;
        }
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot,
            volume_pct,
            pan_pos,
            ..
        } => {
            validate_instrument_slot(*instrument_slot)?;
            if let Some(value) = volume_pct {
                ensure_finite(*value, "instrument volume")?;
            }
            let _ = pan_pos;
        }
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index,
            volume_pct: Some(value),
            ..
        } => {
            validate_fx_bus(*bus_index)?;
            ensure_finite(*value, "FX bus volume")?;
        }
        RuntimeAudioCommand::SetFxBusMixer { bus_index, .. } => validate_fx_bus(*bus_index)?,
        RuntimeAudioCommand::SetFxBusParam {
            bus_index,
            slot_index,
            value,
            ..
        } => {
            validate_fx_bus_slot(*bus_index, *slot_index)?;
            ensure_finite(*value, "FX parameter")?;
        }
        RuntimeAudioCommand::SetGlobalFxParam {
            slot_index, value, ..
        } => {
            validate_global_fx_slot(*slot_index)?;
            ensure_finite(*value, "FX parameter")?;
        }
        RuntimeAudioCommand::SamplePreview {
            instrument_slot, ..
        } => validate_instrument_slot(*instrument_slot)?,
        _ => {}
    }
    Ok(())
}

pub(super) fn index_u8(index: usize, label: &str) -> Result<u8, RuntimeAdapterError> {
    u8::try_from(index)
        .map_err(|_| invalid_audio_command(format!("{label} is out of range: {index}")))
}

fn validate_instrument_slot(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < INSTRUMENT_SLOT_COUNT)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("instrument slot is out of range: {index}")))
}

fn validate_fx_bus(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < BUS_COUNT)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("FX bus index is out of range: {index}")))
}

fn validate_fx_bus_slot(bus_index: usize, slot_index: usize) -> Result<(), RuntimeAdapterError> {
    validate_fx_bus(bus_index)?;
    (slot_index < BUS_SLOTS_PER_BUS)
        .then_some(())
        .ok_or_else(|| {
            invalid_audio_command(format!("FX slot index is out of range: {slot_index}"))
        })
}

fn validate_global_fx_slot(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < GLOBAL_FX_SLOT_COUNT)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("global FX slot is out of range: {index}")))
}

fn validate_momentary_target(target: &RuntimeMomentaryFxTarget) -> Result<(), RuntimeAdapterError> {
    match target {
        RuntimeMomentaryFxTarget::Global => Ok(()),
        RuntimeMomentaryFxTarget::FxBus { index } => validate_fx_bus(*index),
        RuntimeMomentaryFxTarget::Instrument { index } => validate_instrument_slot(*index),
    }
}

fn ensure_finite(value: f32, label: &str) -> Result<(), RuntimeAdapterError> {
    value
        .is_finite()
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("{label} must be finite")))
}

fn validate_params(
    params: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Result<(), RuntimeAdapterError> {
    for (key, value) in params {
        if !value
            .as_f64()
            .map(|number| (number as f32).is_finite())
            .unwrap_or(false)
        {
            return Err(invalid_audio_command(format!(
                "FX parameter `{key}` must be finite"
            )));
        }
    }
    Ok(())
}

pub(super) fn momentary_fx_target(target: &RuntimeMomentaryFxTarget) -> MomentaryFxTarget {
    match target {
        RuntimeMomentaryFxTarget::Global => MomentaryFxTarget::Global,
        RuntimeMomentaryFxTarget::FxBus { index } => MomentaryFxTarget::FxBus { index: *index },
        RuntimeMomentaryFxTarget::Instrument { index } => {
            MomentaryFxTarget::Instrument { index: *index }
        }
    }
}

pub(super) fn invalid_audio_command(message: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Audio,
        RuntimeErrorCode::InvalidPayload,
        RuntimeOperation::AudioCommand,
        Some(message),
    ))
}

pub(super) fn audio_queue_failure(message: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Audio,
        RuntimeErrorCode::OperationFailed,
        RuntimeOperation::AudioCommand,
        Some(message),
    ))
}
