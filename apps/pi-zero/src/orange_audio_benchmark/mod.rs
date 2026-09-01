mod cli;
mod finalization;
mod metrics;
mod phase;
mod probe;
mod release;
mod schema;
mod stream;

use crate::dsp_scenarios::LiveScenarioSpec;
use cli::{parse, BenchmarkConfig};
use cpal::traits::StreamTrait;
use finalization::{finalize, request_profile_snapshot, validate_profile_state, RunState};
use metrics::{CallbackMetrics, CallbackMetricsSnapshot};
use phase::MeasurementPhase;
use rodio_engine_source::{event_queue, EngineEvent, EngineEventSender};
use schema::{atomic_write_json, readiness, BenchmarkProgress};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const SAMPLE_RATE: u32 = realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
const PROFILE_TIMEOUT: Duration = Duration::from_secs(5);

pub fn requested() -> bool {
    cli::requested()
}

pub fn run() -> Result<(), String> {
    let config = parse(std::env::args().skip(1))?;
    clear_previous_artifacts(&config)?;
    run_inner(&config)
}

fn run_inner(config: &BenchmarkConfig) -> Result<(), String> {
    let scenario =
        crate::dsp_scenarios::live_scenario(config.scenario.as_str(), SAMPLE_RATE, 600_000)
            .ok_or_else(|| {
                format!(
                    "unknown live benchmark scenario: {}",
                    config.scenario.as_str()
                )
            })?;
    let mut state = RunState::new(
        scenario.expected,
        SAMPLE_RATE,
        config.expected_alsa_period_frames,
        config.output_frames,
    );
    state.invocation_id = match release::required_invocation_id() {
        Ok(invocation_id) => Some(invocation_id),
        Err(error) => {
            state.note_error(error);
            None
        }
    };
    if state.errors.is_empty() {
        if let Err(error) = execute_benchmark(config, scenario, &mut state) {
            state.note_error(error);
        }
    }
    finalize(config, &mut state)
}

fn execute_benchmark(
    config: &BenchmarkConfig,
    scenario: LiveScenarioSpec,
    state: &mut RunState,
) -> Result<(), String> {
    let empty = CallbackMetricsSnapshot::default();
    write_progress(config, "prepared", 0, 0, &empty)?;
    let (sender, receiver) = event_queue();
    let built = stream::build(
        receiver,
        config.output_frames,
        config.internal_frames,
        state.metrics.clone(),
        state.profile_probe.clone(),
        state.phase_control.clone(),
    )?;
    state.install_stream(built);
    state.stream_started = true;
    state
        .stream
        .as_ref()
        .expect("stream was installed before playback")
        .stream
        .play()
        .map_err(|error| format!("failed to start Orange benchmark stream: {error}"))?;
    let stream = state
        .stream
        .as_ref()
        .expect("stream was installed before scheduler qualification");
    crate::audio_priority::qualify_callback_scheduler(
        "Orange benchmark",
        &stream.scheduler,
        Duration::from_millis(250),
    )?;
    state.scheduler_qualified = true;
    wait_for_geometry_stable(&state.metrics, config.output_frames)?;
    let readiness_metrics = state.metrics.snapshot();
    if readiness_metrics.terminal_error
        || readiness_metrics.lifetime_callback_frame_sample_count < 3
        || readiness_metrics.lifetime_callback_frames_min == 0
        || readiness_metrics.lifetime_callback_frames_max > config.output_frames
        || readiness_metrics.lifetime_invalid_callback_frame_count > 0
        || readiness_metrics.post_mute_nonzero_samples > 0
    {
        return Err("callback geometry was not stable at readiness publication".into());
    }
    let invocation_id = state
        .invocation_id
        .as_deref()
        .ok_or_else(|| "INVOCATION_ID is missing before readiness publication".to_string())?;
    let readiness_artifact = readiness(
        config,
        invocation_id,
        &stream.sample_format,
        stream.channels,
        stream.sample_rate,
        &readiness_metrics,
    );
    atomic_write_json(&config.readiness_path, &readiness_artifact)?;
    write_progress(config, "ready", 0, 0, &readiness_metrics)?;
    write_progress(config, "waiting_release", 0, 0, &readiness_metrics)?;
    release::wait_for_release(config, &readiness_artifact, invocation_id)?;
    write_progress(config, "release_accepted", 0, 0, &readiness_metrics)?;
    send_fixture(&sender, scenario.events)?;
    let fixture_snapshot = state.metrics.snapshot();
    write_progress(config, "fixture_injected", 0, 0, &fixture_snapshot)?;
    wait_for_barrier(&sender)?;
    let profile_start = request_profile_snapshot(state)?;
    validate_profile_state(
        &profile_start,
        scenario.expected,
        scenario.expected.expected_voice_admission_drops_start,
    )?;
    state.profile_start = Some(profile_start);
    write_progress(config, "fixture_validated", 0, 0, &state.metrics.snapshot())?;
    set_phase(state, MeasurementPhase::Disabled)?;
    run_window(config, &state.metrics, "warmup", config.warmup_seconds)?;
    state.metrics.enable_measurement();
    set_phase(state, MeasurementPhase::Measuring)?;
    run_window(
        config,
        &state.metrics,
        "measurement",
        config.measure_seconds,
    )
}

fn set_phase(state: &RunState, phase: MeasurementPhase) -> Result<(), String> {
    let generation = state.phase_control.request(phase);
    state
        .phase_control
        .wait_for_ack(generation, phase, PROFILE_TIMEOUT)
        .map(|_| ())
}

fn clear_previous_artifacts(config: &BenchmarkConfig) -> Result<(), String> {
    for path in [
        &config.result_path,
        &config.progress_path,
        &config.readiness_path,
    ] {
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|error| format!("failed to clear benchmark artifact {path:?}: {error}"))?;
        }
    }
    Ok(())
}

fn send_fixture(sender: &EngineEventSender, events: Vec<EngineEvent>) -> Result<(), String> {
    for event in events {
        sender.send(event).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn wait_for_barrier(sender: &EngineEventSender) -> Result<(), String> {
    let (report_tx, report_rx) = mpsc::sync_channel(1);
    sender
        .send(EngineEvent::ProbeMark {
            sent_at: Instant::now(),
            report_tx,
        })
        .map_err(|error| error.to_string())?;
    report_rx
        .recv_timeout(PROFILE_TIMEOUT)
        .map(|_| ())
        .map_err(|error| format!("fixture probe barrier failed: {error}"))
}

fn wait_for_geometry_stable(metrics: &CallbackMetrics, max_frames: u32) -> Result<(), String> {
    let deadline = Instant::now() + PROFILE_TIMEOUT;
    while Instant::now() < deadline {
        let snapshot = metrics.snapshot();
        if snapshot.terminal_error {
            return Err("callback error occurred before geometry readiness".into());
        }
        if snapshot.lifetime_callback_frame_sample_count >= 3
            && snapshot.lifetime_callback_frames_min > 0
            && snapshot.lifetime_callback_frames_max <= max_frames
            && snapshot.lifetime_invalid_callback_frame_count == 0
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Err("callback geometry did not pin and stabilize before readiness".into())
}

fn run_window(
    config: &BenchmarkConfig,
    metrics: &CallbackMetrics,
    phase: &'static str,
    seconds: u64,
) -> Result<(), String> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(seconds);
    let mut last_heartbeat = metrics.snapshot().lifetime_callback_count;
    let mut stalled_for = 0;
    write_progress(config, phase, 0, seconds, &metrics.snapshot())?;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        std::thread::sleep(remaining.min(Duration::from_secs(1)));
        let elapsed = started.elapsed().as_secs().min(seconds);
        let snapshot = metrics.snapshot();
        write_progress(config, phase, elapsed, seconds, &snapshot)?;
        if snapshot.terminal_error {
            return Err("terminal callback or geometry error".into());
        }
        if snapshot.lifetime_callback_count == last_heartbeat {
            stalled_for += 1;
            if stalled_for >= 2 {
                metrics.mark_terminal();
                return Err("callback heartbeat stalled".into());
            }
        } else {
            stalled_for = 0;
        }
        last_heartbeat = snapshot.lifetime_callback_count;
        if Instant::now() >= deadline {
            break;
        }
    }
    if phase == "measurement" && metrics.snapshot().callback_count == 0 {
        return Err("measurement produced no callbacks".into());
    }
    Ok(())
}

fn write_progress(
    config: &BenchmarkConfig,
    phase: &'static str,
    elapsed_seconds: u64,
    target_seconds: u64,
    metrics: &CallbackMetricsSnapshot,
) -> Result<(), String> {
    atomic_write_json(
        &config.progress_path,
        &BenchmarkProgress::new(config, phase, elapsed_seconds, target_seconds, metrics),
    )
}
