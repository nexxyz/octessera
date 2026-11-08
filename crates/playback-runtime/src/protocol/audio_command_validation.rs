use super::audio::{RuntimeAudioCommand, RuntimeMomentaryFxTarget};
use super::runtime_error::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
};
use crate::is_valid_fx_string_param;
use realtime_engine::synth::{
    normalize_audio_config, normalize_instrument_slot_config, validate_fm_param_path,
    validate_fx_type, validate_momentary_fx_type, validate_pluck_param_path,
    validate_sample_bank_param_path, validate_synth_param_path, DrumParamId, BUS_COUNT,
    BUS_SLOTS_PER_BUS, GLOBAL_FX_SLOT_COUNT, INSTRUMENT_SLOT_COUNT, SAMPLE_SLOTS_PER_INSTRUMENT,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub fn validate_audio_command(command: &RuntimeAudioCommand) -> Result<(), RuntimeAdapterError> {
    match command {
        RuntimeAudioCommand::SetAudioConfig { config, .. } => {
            normalize_audio_config(config).map_err(invalid_audio_command)?;
        }
        RuntimeAudioCommand::SetDspConfig { .. } => {}
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
            normalize_instrument_slot_config(config).map_err(invalid_audio_command)?;
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
            validate_params(fx_type, params)?;
        }
        RuntimeAudioCommand::SetGlobalFxSlot {
            slot_index,
            fx_type,
            params,
            ..
        } => {
            validate_global_fx_slot(*slot_index)?;
            validate_fx_type(fx_type).map_err(invalid_audio_command)?;
            validate_params(fx_type, params)?;
        }
        RuntimeAudioCommand::MomentaryFxStart {
            fx_type,
            params,
            target,
            ..
        } => {
            validate_momentary_fx_type(fx_type).map_err(invalid_audio_command)?;
            validate_params(fx_type, params)?;
            validate_momentary_target(target)?;
        }
        RuntimeAudioCommand::MomentaryFxUpdate { params, .. } => validate_params("", params)?,
        RuntimeAudioCommand::MomentaryFxStop { .. } => {}
        RuntimeAudioCommand::SetMasterVolume { volume_pct, .. } => {
            ensure_finite(*volume_pct, "master volume")?;
        }
        RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot,
            volume_pct,
            ..
        } => {
            validate_instrument_slot(*instrument_slot)?;
            if let Some(value) = volume_pct {
                ensure_finite(*value, "instrument volume")?;
            }
        }
        RuntimeAudioCommand::SetFxBusMixer {
            bus_index,
            volume_pct,
            ..
        } => {
            validate_fx_bus(*bus_index)?;
            if let Some(value) = volume_pct {
                ensure_finite(*value, "FX bus volume")?;
            }
        }
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
            instrument_slot,
            sample_slot,
            ..
        } => {
            validate_instrument_slot(*instrument_slot)?;
            if *sample_slot >= SAMPLE_SLOTS_PER_INSTRUMENT {
                return Err(invalid_audio_command(format!(
                    "sample slot is out of range: {sample_slot}"
                )));
            }
        }
    }
    Ok(())
}

pub fn invalid_audio_command(message: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Audio,
        RuntimeErrorCode::InvalidPayload,
        RuntimeOperation::AudioCommand,
        Some(message),
    ))
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
    fx_type: &str,
    params: &BTreeMap<String, Value>,
) -> Result<(), RuntimeAdapterError> {
    for (key, value) in params {
        if let Some(text) = value.as_str() {
            if is_valid_fx_string_param(fx_type, key, text) {
                continue;
            }
            return Err(invalid_audio_command(format!(
                "FX parameter `{key}` is not valid for {fx_type}"
            )));
        }
        if !value
            .as_f64()
            .is_some_and(|number| (number as f32).is_finite())
        {
            return Err(invalid_audio_command(format!(
                "FX parameter `{key}` must be finite"
            )));
        }
    }
    Ok(())
}
