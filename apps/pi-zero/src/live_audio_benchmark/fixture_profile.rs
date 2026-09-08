use super::cli::BenchmarkExecutorMode;
use super::finalization::{request_profile_snapshot, validate_profile_state, RunState};
use super::{ensure_stream_runtime_health, set_phase, PROFILE_TIMEOUT};
use crate::audio::AudioStreamHealth;
use crate::dsp_scenarios::ExpectedLiveState;
use realtime_engine::synth::{SourceWorkerHealth, SynthProfileSnapshot};
use rodio_engine_source::{EngineEvent, EngineEventSender, PersistentOutputCounters};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub(super) fn capture_disabled_phase_counters(
    state: &RunState,
) -> Result<PersistentOutputCounters, String> {
    let phase = set_phase(state, super::phase::MeasurementPhase::Disabled)?;
    state.phase_boundary_counters(phase.generation)
}

pub(super) fn validate_fixture_profile(
    config: &super::cli::BenchmarkConfig,
    expected: ExpectedLiveState,
    sender: &EngineEventSender,
    state: &RunState,
    fixture_counters_before: Option<PersistentOutputCounters>,
) -> Result<SynthProfileSnapshot, String> {
    let profile = read_fixture_profile(config.executor_mode, sender, state)?;
    if let Err(error) = validate_profile_state(
        &profile,
        expected,
        expected.expected_voice_admission_drops_start,
    ) {
        return retry_fixture_profile_after_recovered_miss(
            error,
            config.continue_on_recovered_miss,
            fixture_counters_before,
            || capture_disabled_phase_counters(state),
            || wait_for_profile_worker_recovery(state),
            || validate_fixture_profile_once(config.executor_mode, expected, sender, state),
        );
    }
    Ok(profile)
}

fn validate_fixture_profile_once(
    executor_mode: BenchmarkExecutorMode,
    expected: ExpectedLiveState,
    sender: &EngineEventSender,
    state: &RunState,
) -> Result<SynthProfileSnapshot, String> {
    let profile = read_fixture_profile(executor_mode, sender, state)?;
    validate_profile_state(
        &profile,
        expected,
        expected.expected_voice_admission_drops_start,
    )?;
    Ok(profile)
}

fn read_fixture_profile(
    executor_mode: BenchmarkExecutorMode,
    sender: &EngineEventSender,
    state: &RunState,
) -> Result<SynthProfileSnapshot, String> {
    wait_for_fixture_profile_barriers(executor_mode, || {
        wait_for_barrier(
            sender,
            state.stream.as_ref().expect("benchmark stream").health(),
        )
    })?;
    request_profile_snapshot(state)
}

fn retry_fixture_profile_after_recovered_miss(
    initial_error: String,
    continue_on_recovered_miss: bool,
    counters_before: Option<PersistentOutputCounters>,
    capture_after: impl FnOnce() -> Result<PersistentOutputCounters, String>,
    wait_for_recovery: impl FnOnce() -> Result<(), String>,
    retry_profile: impl FnOnce() -> Result<SynthProfileSnapshot, String>,
) -> Result<SynthProfileSnapshot, String> {
    if !continue_on_recovered_miss {
        return Err(initial_error);
    }
    let Some(counters_before) = counters_before else {
        return Err("recovered-miss boundary was not captured".into());
    };
    let counters_after = capture_after()?;
    if counters_after.deadline_misses <= counters_before.deadline_misses {
        return Err(initial_error);
    }
    wait_for_recovery()?;
    retry_profile()
}

fn wait_for_profile_worker_recovery(state: &RunState) -> Result<(), String> {
    let deadline = Instant::now() + PROFILE_TIMEOUT;
    loop {
        ensure_stream_runtime_health(state)?;
        let worker_health = state.current_worker_health();
        if worker_health == SourceWorkerHealth::Healthy {
            return Ok(());
        }
        if worker_health.is_terminal() {
            state
                .stream
                .as_ref()
                .expect("benchmark stream")
                .report_worker_terminal();
            return Err("benchmark DSP worker entered a terminal health state".into());
        }
        if state.metrics.snapshot().terminal_error {
            return Err("callback error occurred while waiting for profile snapshot".into());
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Err("benchmark DSP worker did not recover before fixture profile retry".into());
        };
        std::thread::sleep(remaining.min(Duration::from_millis(5)));
    }
}

pub(super) fn send_fixture(
    sender: &EngineEventSender,
    events: Vec<EngineEvent>,
) -> Result<(), String> {
    for event in events {
        sender.send(event).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn wait_for_barrier(sender: &EngineEventSender, health: &AudioStreamHealth) -> Result<(), String> {
    wait_for_barrier_report(send_probe(sender)?, health)
}

fn send_probe(sender: &EngineEventSender) -> Result<mpsc::Receiver<u128>, String> {
    let (report_tx, report_rx) = mpsc::sync_channel(1);
    sender
        .send(EngineEvent::ProbeMark {
            sent_at: Instant::now(),
            report_tx,
        })
        .map_err(|error| error.to_string())?;
    Ok(report_rx)
}

fn wait_for_barrier_report(
    report_rx: mpsc::Receiver<u128>,
    health: &AudioStreamHealth,
) -> Result<(), String> {
    let deadline = Instant::now() + PROFILE_TIMEOUT;
    loop {
        if health.runtime_status() == crate::audio::AudioStreamStatus::Terminal {
            health.log_worker_terminal_once();
            return Err("benchmark DSP worker entered a terminal health state".into());
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Err("fixture probe barrier failed: timed out".into());
        };
        match report_rx.recv_timeout(remaining.min(Duration::from_millis(50))) {
            Ok(_) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(error) => return Err(format!("fixture probe barrier failed: {error}")),
        }
    }
}

fn wait_for_fixture_profile_barriers(
    executor_mode: BenchmarkExecutorMode,
    mut wait_for_barrier: impl FnMut() -> Result<(), String>,
) -> Result<(), String> {
    wait_for_barrier()?;
    if executor_mode == BenchmarkExecutorMode::RoutingTreePersistent {
        wait_for_barrier()?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "fixture_barrier_tests.rs"]
mod fixture_barrier_tests;
