use super::prep_results::{AudioPrepError, PreparedAudioConfig};
use crate::audio::AudioService;
use crate::audio_config_parse::{parse_audio_config, sample_banks, sample_signature};
use realtime_engine::synth::{
    prepare_audio_config as prepare_engine_audio_config, DEFAULT_AUDIO_SAMPLE_RATE,
};
use rodio_engine_source::EngineEvent;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

pub(super) fn prepare_audio_config(
    audio: &AudioService,
    revision: u64,
    generation: u64,
    config: serde_json::Value,
    samples_dir: PathBuf,
) -> Result<PreparedAudioConfig, AudioPrepError> {
    let parsed = parse_audio_config(&config).map_err(AudioPrepError::InvalidConfig)?;
    let next_signature = sample_signature(&parsed);
    let should_update_sample_banks = {
        let current = audio
            .sample_bank_signature
            .lock()
            .map_err(|_| AudioPrepError::Failed("sample bank signature lock failed".into()))?;
        *current != next_signature
    };
    ensure_current_audio_revision(audio.config_revision.load(Ordering::Acquire), revision)?;
    let sample_banks = if should_update_sample_banks {
        Some(sample_banks(&parsed, &samples_dir, audio).map_err(AudioPrepError::Sample)?)
    } else {
        None
    };
    ensure_current_audio_revision(audio.config_revision.load(Ordering::Acquire), revision)?;
    Ok(PreparedAudioConfig {
        event: EngineEvent::SetPreparedAudioConfig {
            generation,
            config: prepare_engine_audio_config(
                parsed.instruments_config(),
                sample_banks,
                parsed.voice_stealing_mode,
                DEFAULT_AUDIO_SAMPLE_RATE,
            ),
        },
        sample_signature: should_update_sample_banks.then_some(next_signature),
    })
}

pub(super) fn ensure_current_audio_revision(
    current: u64,
    expected: u64,
) -> Result<(), AudioPrepError> {
    (current == expected)
        .then_some(())
        .ok_or(AudioPrepError::Superseded)
}

pub(super) fn apply_prepared_audio_config(
    audio: &AudioService,
    prepared: PreparedAudioConfig,
    generation: u64,
) -> Result<(), String> {
    audio.broadcast(prepared.event)?;
    if let Some(signature) = prepared.sample_signature {
        let mut current = audio
            .sample_bank_signature
            .lock()
            .map_err(|_| "sample bank signature lock failed".to_string())?;
        *current = signature;
    }
    let mut generations = audio
        .generations
        .lock()
        .map_err(|_| "audio generation state lock failed".to_string())?;
    generations.full = generations.full.max(generation);
    let full_generation = generations.full;
    for current in &mut generations.instrument {
        *current = (*current).max(full_generation);
    }
    for current in &mut generations.sample {
        *current = (*current).max(full_generation);
    }
    Ok(())
}
