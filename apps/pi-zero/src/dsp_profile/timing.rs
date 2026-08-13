use crate::dsp_scenarios::ScenarioSpec;
use playback_runtime::{CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, SyncSource};
use realtime_engine::synth::{DEFAULT_AUDIO_BLOCK_FRAMES, DEFAULT_AUDIO_SAMPLE_RATE};
use rodio_engine_source::{event_queue, EngineSource};
use serde_json::json;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::process::Command;
use std::time::Instant;

#[path = "engine_event_clone.rs"]
mod engine_event_clone;

use engine_event_clone::clone_event;

const MIN_BLOCK_FRAMES: usize = 32;
const MAX_BLOCK_FRAMES: usize = 2_048;
const RUNTIME_WARMUP_BLOCKS: usize = 8;

pub fn profile_block_frames() -> usize {
    env_usize("OCTESSERA_AUDIO_BLOCK_FRAMES")
        .unwrap_or(DEFAULT_AUDIO_BLOCK_FRAMES)
        .clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES)
}

pub fn profile_measure_frames(block_frames: usize) -> usize {
    env_usize("OCTESSERA_PI_PROFILE_MEASURE_FRAMES")
        .unwrap_or(block_frames)
        .clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES)
}

pub fn profile_sample_rate() -> u32 {
    env_usize("OCTESSERA_PI_PROFILE_SAMPLE_RATE")
        .map(|value| value as u32)
        .unwrap_or(DEFAULT_AUDIO_SAMPLE_RATE)
}

pub fn measure_engine_source(
    scenario: &ScenarioSpec,
    sample_rate: u32,
    internal_block_frames: usize,
    measure_frames: usize,
    blocks: usize,
) -> Result<Vec<f64>, String> {
    let (tx, rx) = event_queue();
    for event in &scenario.events {
        tx.send(clone_event(event))
            .map_err(|error| format!("engine event send failed: {error}"))?;
    }
    let mut source = EngineSource::with_block_frames(rx, sample_rate, internal_block_frames);
    let samples_per_block = measure_frames * 2;
    let block_seconds = measure_frames as f64 / sample_rate as f64;
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
    let message_blocks = runtime_step_message_blocks(&scenario.name, block_frames, blocks);
    for messages in message_blocks.iter().take(RUNTIME_WARMUP_BLOCKS) {
        for message in messages {
            let _ = runner.send(message.clone())?;
        }
    }
    let mut timings = Vec::with_capacity(blocks);
    for messages in message_blocks.into_iter().skip(RUNTIME_WARMUP_BLOCKS) {
        let start = Instant::now();
        for message in messages {
            let _ = runner.send(message)?;
        }
        timings.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    Ok(timings)
}

fn prime_runtime_scenario(runner: &mut NativeRunner, scenario: &str) -> Result<(), String> {
    if matches!(
        scenario,
        "runtime_noteoff_queue_stress" | "runtime_noteoff_snapshot_stress"
    ) {
        for x in 0..8 {
            let _ = runner.send(HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": x, "y": 0 }),
                request_snapshot: None,
            })?;
        }
    }
    if scenario == "menu_snapshot_only" {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
            request_snapshot: None,
        })?;
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })?;
    }
    if scenario == "runtime_snapshot_no_menu_change" {
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })?;
        let _ = runner.send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 0, "y": 0 }),
            request_snapshot: None,
        })?;
    }
    Ok(())
}

fn runtime_step_message_blocks(
    scenario: &str,
    block_frames: usize,
    measured_blocks: usize,
) -> Vec<Vec<HostMessage>> {
    (0..measured_blocks + RUNTIME_WARMUP_BLOCKS)
        .map(|block| runtime_step_messages(scenario, block, block_frames))
        .collect()
}

fn runtime_step_messages(scenario: &str, block: usize, block_frames: usize) -> Vec<HostMessage> {
    match scenario {
        "snapshot_only_idle" | "runtime_snapshot_no_menu_change" | "menu_snapshot_only" => {
            vec![pulse_message(block_frames, Some(true))]
        }
        "dense_scan_transform_events" => dense_scan_messages(block, block_frames, None),
        "dense_scan_transform_snapshot" => dense_scan_messages(block, block_frames, Some(true)),
        "menu_nav_no_snapshot" => menu_nav_messages(block, block_frames, None),
        "menu_snapshot_nav_stress" => menu_nav_messages(block, block_frames, Some(true)),
        "runtime_noteoff_queue_stress" => noteoff_queue_messages(block, block_frames, None),
        "runtime_noteoff_snapshot_stress" => {
            noteoff_queue_messages(block, block_frames, Some(block.is_multiple_of(2)))
        }
        _ => vec![pulse_message(block_frames, None)],
    }
}

fn dense_scan_messages(
    block: usize,
    block_frames: usize,
    request_snapshot: Option<bool>,
) -> Vec<HostMessage> {
    let x = (block % 8) as u8;
    let y = ((block / 8) % 8) as u8;
    vec![
        HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": x, "y": y }),
            request_snapshot: None,
        },
        pulse_message(block_frames, request_snapshot),
        HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": x, "y": y }),
            request_snapshot: None,
        },
    ]
}

fn menu_nav_messages(
    block: usize,
    block_frames: usize,
    request_snapshot: Option<bool>,
) -> Vec<HostMessage> {
    let delta = if block.is_multiple_of(2) { 1 } else { -1 };
    vec![
        HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
            request_snapshot: None,
        },
        HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        },
        pulse_message(block_frames, request_snapshot),
    ]
}

fn noteoff_queue_messages(
    block: usize,
    block_frames: usize,
    request_snapshot: Option<bool>,
) -> Vec<HostMessage> {
    let x = (block % 8) as u8;
    vec![
        HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": x, "y": 0 }),
            request_snapshot: None,
        },
        pulse_message(block_frames, request_snapshot),
        HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": x, "y": 0 }),
            request_snapshot: None,
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

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub fn vcgencmd_output() -> Vec<(String, String)> {
    ["measure_temp", "get_throttled"]
        .into_iter()
        .filter_map(|metric| {
            run_command("vcgencmd", &[metric]).map(|value| (metric.to_string(), value))
        })
        .collect()
}

pub fn profile_system_output() -> Vec<(String, String)> {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    {
        orange_system_output()
    }
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    {
        vcgencmd_output()
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn orange_system_output() -> Vec<(String, String)> {
    let mut output = vec![(
        "board_profile".to_string(),
        octessera_pi::board_profile::BOARD_PROFILE_ID.to_string(),
    )];
    if let Some(value) = read_system_file("/proc/loadavg") {
        output.push(("loadavg".into(), value));
    }
    if let Some(value) = read_mem_available() {
        output.push(("mem_available_kb".into(), value));
    }
    if let Some(value) = read_system_file("/sys/class/thermal/thermal_zone0/temp") {
        output.push(("thermal_zone0_millicelsius".into(), value));
    }
    if let Some(value) = read_system_file("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq") {
        output.push(("cpu0_scaling_cur_freq_khz".into(), value));
    }
    output
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn read_system_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn read_mem_available() -> Option<String> {
    std::fs::read_to_string("/proc/meminfo")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("MemAvailable:")?
                .split_whitespace()
                .next()
        })
        .map(str::to_string)
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
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
