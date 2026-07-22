mod audio_sample_decode;

use crate::audio::AudioService;
pub(crate) use audio_sample_decode::decode_sample_file;
use realtime_engine::synth::{
    normalize_audio_config, normalize_instrument_slot_config, InstrumentSlotConfig,
    NormalizedAudioConfig, SampleBankConfig, SampleBuffer, SampleSlotConfig, INSTRUMENT_SLOT_COUNT,
    SAMPLE_SLOTS_PER_INSTRUMENT,
};
use std::path::{Component, Path, PathBuf};

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
) -> Vec<SampleBankConfig> {
    config
        .instruments
        .iter()
        .take(INSTRUMENT_SLOT_COUNT)
        .map(|instrument| {
            let Some(sample) = instrument.active_sample() else {
                return SampleBankConfig::default();
            };
            let mut slots = vec![SampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT];
            for (index, path) in sample.slots.iter().enumerate() {
                let Some(path) = path
                    .as_deref()
                    .and_then(|path| resolve_sample_path(samples_dir, path))
                else {
                    continue;
                };
                slots[index].buffer = cached_sample_buffer(audio, &path);
            }
            SampleBankConfig {
                slots,
                tune_semis: sample.tune_semis,
                gain_pct: sample.gain_pct,
                velocity_sensitivity_pct: sample.velocity_sensitivity_pct,
                filter_cutoff_hz: sample.filter_cutoff_hz,
                filter_resonance: sample.filter_resonance,
            }
        })
        .collect()
}

pub(crate) fn sample_signature(config: &NormalizedAudioConfig) -> String {
    config.sample_bank_signature()
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

fn cached_sample_buffer(audio: &AudioService, path: &Path) -> Option<SampleBuffer> {
    let key = path.to_string_lossy().to_string();
    if let Ok(cache) = audio.sample_cache.lock() {
        if let Some(buffer) = cache.get(&key) {
            return Some(buffer.clone());
        }
    } else {
        return None;
    }
    let buffer = decode_sample_file(path)?;
    if let Ok(mut cache) = audio.sample_cache.lock() {
        cache.insert(key, buffer.clone());
    }
    Some(buffer)
}

fn pi_relative_sample_path(path: &str) -> &str {
    path.strip_prefix("samples/")
        .or_else(|| path.strip_prefix(r"samples\"))
        .unwrap_or(path)
}
