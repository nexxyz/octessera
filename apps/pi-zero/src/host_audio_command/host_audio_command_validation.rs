use playback_runtime::{
    invalid_audio_command, RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorFacts, RuntimeMomentaryFxTarget, RuntimeOperation,
};
use realtime_engine::synth::MomentaryFxTarget;

pub(super) fn index_u8(index: usize, label: &str) -> Result<u8, RuntimeAdapterError> {
    u8::try_from(index)
        .map_err(|_| invalid_audio_command(format!("{label} is out of range: {index}")))
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

pub(super) fn audio_queue_failure(message: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Audio,
        RuntimeErrorCode::OperationFailed,
        RuntimeOperation::AudioCommand,
        Some(message),
    ))
}
