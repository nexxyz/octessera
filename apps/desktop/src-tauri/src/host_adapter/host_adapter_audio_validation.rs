use playback_runtime::{RuntimeAdapterError, RuntimeMomentaryFxTarget};
use realtime_engine::synth::{
    BUS_COUNT, BUS_SLOTS_PER_BUS, GLOBAL_FX_SLOT_COUNT, INSTRUMENT_SLOT_COUNT,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn invalid_audio_command(message: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(playback_runtime::RuntimeErrorFacts::new(
        playback_runtime::RuntimeErrorDomain::Audio,
        playback_runtime::RuntimeErrorCode::InvalidPayload,
        playback_runtime::RuntimeOperation::AudioCommand,
        Some(message),
    ))
}

pub(super) fn ensure_finite(value: f32, name: &str) -> Result<(), RuntimeAdapterError> {
    value
        .is_finite()
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("{name} must be finite")))
}

pub(super) fn ensure_optional_finite(
    value: Option<f32>,
    name: &str,
) -> Result<(), RuntimeAdapterError> {
    value.map_or(Ok(()), |value| ensure_finite(value, name))
}

pub(super) fn validate_param_values(
    fx_type: &str,
    params: &BTreeMap<String, Value>,
) -> Result<(), RuntimeAdapterError> {
    params.iter().try_for_each(|(key, value)| {
        if let Some(text) = value.as_str() {
            return playback_runtime::is_valid_fx_string_param(fx_type, key, text)
                .then_some(())
                .ok_or_else(|| {
                    invalid_audio_command(format!(
                        "FX parameter `{key}` is not valid for {fx_type}"
                    ))
                });
        }
        let finite = value
            .as_f64()
            .map(|value| (value as f32).is_finite())
            .unwrap_or(false);
        finite
            .then_some(())
            .ok_or_else(|| invalid_audio_command(format!("FX parameter `{key}` must be finite")))
    })
}

pub(super) fn validate_instrument_slot(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < INSTRUMENT_SLOT_COUNT)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("invalid instrument slot {index}")))
}

pub(super) fn validate_bus_index(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < BUS_COUNT)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("invalid FX bus {index}")))
}

pub(super) fn validate_bus_slot(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < BUS_SLOTS_PER_BUS)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("invalid FX bus slot {index}")))
}

pub(super) fn validate_global_slot(index: usize) -> Result<(), RuntimeAdapterError> {
    (index < GLOBAL_FX_SLOT_COUNT)
        .then_some(())
        .ok_or_else(|| invalid_audio_command(format!("invalid global FX slot {index}")))
}

pub(super) fn validate_momentary_target(
    target: &RuntimeMomentaryFxTarget,
) -> Result<(), RuntimeAdapterError> {
    match target {
        RuntimeMomentaryFxTarget::Global => Ok(()),
        RuntimeMomentaryFxTarget::FxBus { index } => validate_bus_index(*index),
        RuntimeMomentaryFxTarget::Instrument { index } => validate_instrument_slot(*index),
    }
}
