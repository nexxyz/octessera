use super::scenarios::ScenarioSpec;
use playback_runtime::{CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, SyncSource};
use realtime_engine::synth::{DEFAULT_AUDIO_BLOCK_FRAMES, DEFAULT_AUDIO_SAMPLE_RATE};
use rodio_engine_source::EngineSource;
use serde_json::json;
use std::process::Command;
use std::sync::mpsc;
use std::time::Instant;

const MIN_BLOCK_FRAMES: usize = 32;
const MAX_BLOCK_FRAMES: usize = 2_048;

pub fn profile_block_frames() -> usize {
    env_usize("CELLSYMPHONY_AUDIO_BLOCK_FRAMES")
        .unwrap_or(DEFAULT_AUDIO_BLOCK_FRAMES)
        .clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES)
}

pub fn profile_sample_rate() -> u32 {
    env_usize("CELLSYMPHONY_PI_PROFILE_SAMPLE_RATE")
        .map(|value| value as u32)
        .unwrap_or(DEFAULT_AUDIO_SAMPLE_RATE)
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
    scenario: &ScenarioSpec,
    _sample_rate: u32,
    block_frames: usize,
    blocks: usize,
) -> Result<Vec<f64>, String> {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default())?;
    prime_runtime_scenario(&mut runner, &scenario.name)?;
    let mut timings = Vec::with_capacity(blocks);
    for block in 0..blocks {
        let start = Instant::now();
        for message in runtime_step_messages(&scenario.name, block, block_frames) {
            let _ = runner.send(message)?;
        }
        timings.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    Ok(timings)
}

fn prime_runtime_scenario(runner: &mut NativeRunner, scenario: &str) -> Result<(), String> {
    if scenario == "runtime_noteoff_queue_stress" {
        for x in 0..8 {
            let _ = runner.send(HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": x, "y": 0 }),
            })?;
        }
    }
    Ok(())
}

fn runtime_step_messages(scenario: &str, block: usize, block_frames: usize) -> Vec<HostMessage> {
    match scenario {
        "dense_scan_transform_events" => dense_scan_messages(block, block_frames),
        "menu_snapshot_nav_stress" => menu_nav_messages(block, block_frames),
        "runtime_noteoff_queue_stress" => noteoff_queue_messages(block, block_frames),
        _ => vec![pulse_message(block_frames, None)],
    }
}

fn dense_scan_messages(block: usize, block_frames: usize) -> Vec<HostMessage> {
    let x = (block % 8) as u8;
    let y = ((block / 8) % 8) as u8;
    vec![
        HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": x, "y": y }),
        },
        pulse_message(block_frames, Some(true)),
        HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": x, "y": y }),
        },
    ]
}

fn menu_nav_messages(block: usize, block_frames: usize) -> Vec<HostMessage> {
    let delta = if block.is_multiple_of(2) { 1 } else { -1 };
    vec![
        HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
        },
        HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        },
        pulse_message(block_frames, Some(true)),
    ]
}

fn noteoff_queue_messages(block: usize, block_frames: usize) -> Vec<HostMessage> {
    let x = (block % 8) as u8;
    vec![
        HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": x, "y": 0 }),
        },
        pulse_message(block_frames, Some(block.is_multiple_of(2))),
        HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": x, "y": 0 }),
        },
    ]
}

fn pulse_message(block_frames: usize, request_snapshot: Option<bool>) -> HostMessage {
    HostMessage::TransportPulseStep {
        pulses: block_frames.max(1) as u32,
        source: SyncSource::Internal,
        at_ppqn_pulse: None,
        request_snapshot,
    }
}

pub fn vcgencmd_output() -> Vec<(String, String)> {
    ["measure_temp", "get_throttled"]
        .into_iter()
        .filter_map(|metric| {
            run_command("vcgencmd", &[metric]).map(|value| (metric.to_string(), value))
        })
        .collect()
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
