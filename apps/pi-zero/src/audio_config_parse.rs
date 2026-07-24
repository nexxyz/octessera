mod audio_sample_decode;

use crate::audio::AudioService;
pub(crate) use audio_sample_decode::decode_sample_file;
use playback_runtime::RuntimeErrorCode;
use realtime_engine::synth::{
    normalize_audio_config, normalize_instrument_slot_config, InstrumentSlotConfig,
    NormalizedAudioConfig, SampleBankConfig, SampleBuffer, SampleSlotConfig, INSTRUMENT_SLOT_COUNT,
    SAMPLE_SLOTS_PER_INSTRUMENT,
};
use rodio_engine_source::EngineEvent;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SampleLoadError {
    Unresolved(String),
    Undecodable(String),
}

impl SampleLoadError {
    pub(crate) fn code(&self) -> RuntimeErrorCode {
        match self {
            Self::Unresolved(_) => RuntimeErrorCode::NotFound,
            Self::Undecodable(_) => RuntimeErrorCode::OperationFailed,
        }
    }

    pub(crate) fn message(&self) -> String {
        match self {
            Self::Unresolved(path) => format!("sample not found: {path}"),
            Self::Undecodable(path) => format!("sample decode failed: {path}"),
        }
    }
}

#[cfg(test)]
#[path = "audio_config_parse_tests.rs"]
mod audio_config_parse_tests;

pub(crate) fn parse_audio_config(
    config: &serde_json::Value,
) -> Result<NormalizedAudioConfig, String> {
    normalize_audio_config(config)
}

pub(crate) fn parse_instrument_slot_config(
    config: &serde_json::Value,
) -> Result<InstrumentSlotConfig, String> {
    Ok(normalize_instrument_slot_config(config)?.slot)
}

pub(crate) fn sample_banks(
    config: &NormalizedAudioConfig,
    samples_dir: &Path,
    audio: &AudioService,
) -> Result<Vec<SampleBankConfig>, SampleLoadError> {
    config
        .instruments
        .iter()
        .take(INSTRUMENT_SLOT_COUNT)
        .map(|instrument| -> Result<SampleBankConfig, SampleLoadError> {
            let Some(sample) = instrument.active_sample() else {
                return Ok(SampleBankConfig::default());
            };
            let mut slots = vec![SampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT];
            for (index, path) in sample.slots.iter().enumerate() {
                let Some(path) = path.as_deref() else {
                    continue;
                };
                let resolved = resolve_sample_path(samples_dir, path)
                    .ok_or_else(|| SampleLoadError::Unresolved(path.into()))?;
                slots[index].buffer = Some(cached_sample_buffer(audio, &resolved, path)?);
            }
            Ok(SampleBankConfig {
                slots,
                tune_semis: sample.tune_semis,
                gain_pct: sample.gain_pct,
                velocity_sensitivity_pct: sample.velocity_sensitivity_pct,
                filter_cutoff_hz: sample.filter_cutoff_hz,
                filter_resonance: sample.filter_resonance,
            })
        })
        .collect()
}

pub(crate) fn sample_signature(config: &NormalizedAudioConfig) -> String {
    config.sample_bank_signature()
}

pub(crate) fn prepare_sample_preview(
    audio: &AudioService,
    instrument_slot: usize,
    path: &str,
    velocity: u8,
    samples_dir: &Path,
) -> Result<EngineEvent, SampleLoadError> {
    let resolved = resolve_sample_path(samples_dir, path)
        .ok_or_else(|| SampleLoadError::Unresolved(path.into()))?;
    let buffer = cached_sample_buffer(audio, &resolved, path)?;
    Ok(EngineEvent::PreviewSample {
        instrument_slot: instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1) as u8,
        buffer,
        velocity,
    })
}

pub(crate) fn resolve_sample_path(samples_dir: &Path, path: &str) -> Option<PathBuf> {
    let relative = Path::new(pi_relative_sample_path(path));
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return None;
    }
    let root = samples_dir.canonicalize().ok()?;
    let target = root.join(relative).canonicalize().ok()?;
    target.starts_with(&root).then_some(target)
}

fn cached_sample_buffer(
    audio: &AudioService,
    path: &Path,
    display_path: &str,
) -> Result<SampleBuffer, SampleLoadError> {
    let key = path.to_string_lossy().to_string();
    if let Ok(cache) = audio.sample_cache.lock() {
        if let Some(buffer) = cache.get(&key) {
            return Ok(buffer.clone());
        }
    } else {
        return Err(SampleLoadError::Undecodable(display_path.into()));
    }
    let buffer = decode_sample_file(path)
        .ok_or_else(|| SampleLoadError::Undecodable(display_path.into()))?;
    if let Ok(mut cache) = audio.sample_cache.lock() {
        cache.insert(key, buffer.clone());
    } else {
        return Err(SampleLoadError::Undecodable(display_path.into()));
    }
    Ok(buffer)
}

fn pi_relative_sample_path(path: &str) -> &str {
    path.strip_prefix("samples/")
        .or_else(|| path.strip_prefix(r"samples\"))
        .unwrap_or(path)
}
