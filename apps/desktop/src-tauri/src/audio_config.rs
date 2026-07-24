use playback_runtime::RuntimeErrorCode;
use realtime_engine::synth::{
    normalize_audio_config, normalize_instrument_slot_config, InstrumentsConfig, SampleBankConfig,
    SampleBuffer, SampleSlotConfig as EngineSampleSlotConfig, INSTRUMENT_SLOT_COUNT,
    SAMPLE_SLOTS_PER_INSTRUMENT,
};
use serde_json::Value;

mod sample_decode;

pub use sample_decode::decode_sample_file;

pub type AudioInstrumentsConfig = realtime_engine::synth::NormalizedAudioConfig;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleBankError {
    InvalidConfig(String),
    Unresolved(String),
    Undecodable(String),
}

impl SampleBankError {
    pub fn code(&self) -> RuntimeErrorCode {
        match self {
            Self::InvalidConfig(_) => RuntimeErrorCode::InvalidPayload,
            Self::Unresolved(_) => RuntimeErrorCode::NotFound,
            Self::Undecodable(_) => RuntimeErrorCode::OperationFailed,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidConfig(message) => message.clone(),
            Self::Unresolved(path) => format!("sample not found: {path}"),
            Self::Undecodable(path) => format!("sample decode failed: {path}"),
        }
    }
}

pub fn normalize_config(config: &Value) -> Result<AudioInstrumentsConfig, String> {
    normalize_audio_config(config)
}

pub fn synth_payload(config: &AudioInstrumentsConfig) -> InstrumentsConfig {
    config.instruments_config()
}

pub fn parse_instrument_slot_config(
    config: &Value,
) -> Result<realtime_engine::synth::InstrumentSlotConfig, String> {
    Ok(normalize_instrument_slot_config(config)?.slot)
}

pub fn synth_slots(config: &AudioInstrumentsConfig) -> [bool; INSTRUMENT_SLOT_COUNT] {
    let mut synth_slots = [false; INSTRUMENT_SLOT_COUNT];
    for (index, instrument) in config.instruments.iter().enumerate() {
        if index >= INSTRUMENT_SLOT_COUNT {
            break;
        }
        synth_slots[index] = instrument.kind == "synth";
    }
    synth_slots
}

pub fn sample_banks(
    config: &AudioInstrumentsConfig,
    resolve_sample: impl Fn(&str) -> Option<String>,
    mut load_sample: impl FnMut(&str) -> Option<SampleBuffer>,
) -> Result<Vec<SampleBankConfig>, SampleBankError> {
    config
        .instruments
        .iter()
        .take(INSTRUMENT_SLOT_COUNT)
        .map(|instrument| sample_bank_for_slot(instrument, &resolve_sample, &mut load_sample))
        .collect()
}

pub fn sample_bank_for_slot_config(
    config: &Value,
    resolve_sample: impl Fn(&str) -> Option<String>,
    load_sample: impl FnMut(&str) -> Option<SampleBuffer>,
) -> Result<Option<SampleBankConfig>, SampleBankError> {
    let instrument =
        normalize_instrument_slot_config(config).map_err(SampleBankError::InvalidConfig)?;
    let bank = sample_bank_for_slot(&instrument, resolve_sample, load_sample)?;
    Ok(instrument.active_sample().is_some().then_some(bank))
}

pub fn sample_bank_signature(config: &AudioInstrumentsConfig) -> String {
    config.sample_bank_signature()
}

fn sample_bank_for_slot(
    instrument: &realtime_engine::synth::NormalizedInstrumentSlot,
    resolve_sample: impl Fn(&str) -> Option<String>,
    mut load_sample: impl FnMut(&str) -> Option<SampleBuffer>,
) -> Result<SampleBankConfig, SampleBankError> {
    let Some(sample) = instrument.active_sample() else {
        return Ok(SampleBankConfig::default());
    };
    let mut slots = vec![EngineSampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT];
    for (index, path) in sample.slots.iter().enumerate() {
        let Some(path) = path else {
            continue;
        };
        let full_path =
            resolve_sample(path).ok_or_else(|| SampleBankError::Unresolved(path.clone()))?;
        slots[index].buffer = Some(
            load_sample(&full_path).ok_or_else(|| SampleBankError::Undecodable(path.clone()))?,
        );
    }
    Ok(SampleBankConfig {
        slots,
        tune_semis: sample.tune_semis,
        gain_pct: sample.gain_pct,
        velocity_sensitivity_pct: sample.velocity_sensitivity_pct,
        filter_cutoff_hz: sample.filter_cutoff_hz,
        filter_resonance: sample.filter_resonance,
    })
}

#[cfg(test)]
#[path = "audio_config_tests.rs"]
mod tests;
