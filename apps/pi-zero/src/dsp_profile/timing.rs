use super::scenarios::ScenarioSpec;
use playback_runtime::{
    HostAdapter, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime,
    RuntimeAudioCommand, RuntimeConfig, RuntimePlatformEffect,
};
use rodio_engine_source::EngineSource;
use std::process::Command;
use std::sync::mpsc;
use std::time::Instant;

const DEFAULT_BLOCK_FRAMES: usize = 128;
const DEFAULT_SAMPLE_RATE: u32 = 48_000;
const MIN_BLOCK_FRAMES: usize = 32;
const MAX_BLOCK_FRAMES: usize = 2_048;

pub fn profile_block_frames() -> usize {
    env_usize("CELLSYMPHONY_AUDIO_BLOCK_FRAMES")
        .unwrap_or(DEFAULT_BLOCK_FRAMES)
        .clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES)
}

pub fn profile_sample_rate() -> u32 {
    env_usize("CELLSYMPHONY_PI_PROFILE_SAMPLE_RATE")
        .map(|value| value as u32)
        .unwrap_or(DEFAULT_SAMPLE_RATE)
}

pub fn measure_engine_source(
    scenario: &ScenarioSpec,
    sample_rate: u32,
    block_frames: usize,
    blocks: usize,
) -> Result<Vec<f64>, String> {
    let (tx, rx) = mpsc::channel();
    for event in &scenario.events {
        tx.send(clone_event(event))
            .map_err(|error| format!("engine event send failed: {error}"))?;
    }
    let mut source = EngineSource::new(rx, sample_rate);
    let samples_per_block = block_frames * 2;
    let block_seconds = block_frames as f64 / sample_rate as f64;
    let mut timings = Vec::with_capacity(blocks);
    for _ in 0..blocks {
        let start = Instant::now();
        for _ in 0..samples_per_block {
            let _ = source.next();
        }
        timings.push(start.elapsed().as_secs_f64() / block_seconds);
    }
    Ok(timings)
}

pub fn measure_runtime_step(
    sample_rate: u32,
    block_frames: usize,
    blocks: usize,
) -> Result<Vec<f64>, String> {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default())?;
    let mut host = NoopHost;
    let step_ms = ((block_frames as f64 / sample_rate as f64) * 1000.0)
        .round()
        .max(1.0) as u64;
    let mut timings = Vec::with_capacity(blocks);
    for _ in 0..blocks {
        let start = Instant::now();
        runtime.advance(step_ms, &mut runner, &mut host)?;
        timings.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    Ok(timings)
}

pub fn vcgencmd_output() -> Vec<(String, String)> {
    ["measure_temp", "get_throttled"]
        .into_iter()
        .filter_map(|metric| {
            run_command("vcgencmd", &[metric]).map(|value| (metric.to_string(), value))
        })
        .collect()
}

struct NoopHost;

impl HostAdapter for NoopHost {
    fn handle_musical_event(&mut self, _event: &platform_core::MusicalEvent) -> Result<(), String> {
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        _effect: &RuntimePlatformEffect,
    ) -> Result<Vec<HostMessage>, String> {
        Ok(Vec::new())
    }

    fn handle_audio_command(&mut self, _command: &RuntimeAudioCommand) -> Result<(), String> {
        Ok(())
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

fn run_command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
}

fn clone_event(event: &rodio_engine_source::EngineEvent) -> rodio_engine_source::EngineEvent {
    match event {
        rodio_engine_source::EngineEvent::NoteOn {
            instrument_slot,
            note,
            velocity,
            duration_ms,
        } => rodio_engine_source::EngineEvent::NoteOn {
            instrument_slot: *instrument_slot,
            note: *note,
            velocity: *velocity,
            duration_ms: *duration_ms,
        },
        rodio_engine_source::EngineEvent::NoteOff {
            instrument_slot,
            note,
        } => rodio_engine_source::EngineEvent::NoteOff {
            instrument_slot: *instrument_slot,
            note: *note,
        },
        rodio_engine_source::EngineEvent::Cc {
            instrument_slot,
            controller,
            value,
        } => rodio_engine_source::EngineEvent::Cc {
            instrument_slot: *instrument_slot,
            controller: *controller,
            value: *value,
        },
        rodio_engine_source::EngineEvent::SetInstruments(config) => {
            rodio_engine_source::EngineEvent::SetInstruments(config.clone())
        }
        rodio_engine_source::EngineEvent::SetSampleBanks(banks) => {
            rodio_engine_source::EngineEvent::SetSampleBanks(banks.clone())
        }
        rodio_engine_source::EngineEvent::PreviewSample {
            instrument_slot,
            buffer,
            velocity,
        } => rodio_engine_source::EngineEvent::PreviewSample {
            instrument_slot: *instrument_slot,
            buffer: buffer.clone(),
            velocity: *velocity,
        },
        rodio_engine_source::EngineEvent::SetVoiceStealingMode(mode) => {
            rodio_engine_source::EngineEvent::SetVoiceStealingMode(*mode)
        }
        rodio_engine_source::EngineEvent::MomentaryFxStart {
            id,
            fx_type,
            params,
            target,
        } => rodio_engine_source::EngineEvent::MomentaryFxStart {
            id: id.clone(),
            fx_type: fx_type.clone(),
            params: params.clone(),
            target: *target,
        },
        rodio_engine_source::EngineEvent::MomentaryFxUpdate { id, params } => {
            rodio_engine_source::EngineEvent::MomentaryFxUpdate {
                id: id.clone(),
                params: params.clone(),
            }
        }
        rodio_engine_source::EngineEvent::MomentaryFxStop { id } => {
            rodio_engine_source::EngineEvent::MomentaryFxStop { id: id.clone() }
        }
    }
}
