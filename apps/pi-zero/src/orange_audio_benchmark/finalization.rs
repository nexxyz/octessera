use super::cli::BenchmarkConfig;
use super::metrics::{CallbackMetrics, CallbackMetricsSnapshot};
use super::phase::{MeasurementControl, MeasurementPhase};
use super::probe::ProfileProbe;
use super::schema::{
    atomic_write_json, BenchmarkProfileSnapshot, BenchmarkProgress, BenchmarkResult,
    BenchmarkWorkerDelta,
};
use super::stream::BenchmarkStream;
use super::worker;
use crate::dsp_scenarios::ExpectedLiveState;
use realtime_engine::synth::SynthProfileSnapshot;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const SHUTDOWN_MARGIN: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
pub struct StreamEvidence {
    pub sample_format: String,
    pub channels: u16,
    pub sample_rate: u32,
    pub workers_effective: bool,
    pub engine_block_frames: usize,
}

pub struct RunState {
    pub metrics: Arc<CallbackMetrics>,
    pub phase_control: Arc<MeasurementControl>,
    pub profile_probe: Arc<ProfileProbe>,
    pub expected: ExpectedLiveState,
    pub stream: Option<BenchmarkStream>,
    pub stream_evidence: Option<StreamEvidence>,
    pub stream_started: bool,
    pub stream_stopped: bool,
    pub scheduler_qualified: bool,
    pub measurement_stop_acknowledged: bool,
    pub profile_start: Option<SynthProfileSnapshot>,
    pub profile_end: Option<SynthProfileSnapshot>,
    pub invocation_id: Option<String>,
    pub errors: Vec<String>,
}

impl RunState {
    pub fn new(
        expected: ExpectedLiveState,
        sample_rate: u32,
        expected_period_frames: u32,
        max_callback_frames: u32,
    ) -> Self {
        Self {
            metrics: Arc::new(CallbackMetrics::new(
                sample_rate,
                expected_period_frames,
                max_callback_frames,
            )),
            phase_control: Arc::new(MeasurementControl::new()),
            profile_probe: Arc::new(ProfileProbe::new()),
            expected,
            stream: None,
            stream_evidence: None,
            stream_started: false,
            stream_stopped: false,
            scheduler_qualified: false,
            measurement_stop_acknowledged: false,
            profile_start: None,
            profile_end: None,
            invocation_id: None,
            errors: Vec::new(),
        }
    }

    pub fn install_stream(&mut self, stream: BenchmarkStream) {
        self.stream_evidence = Some(StreamEvidence {
            sample_format: stream.sample_format.clone(),
            channels: stream.channels,
            sample_rate: stream.sample_rate,
            workers_effective: stream.workers_effective,
            engine_block_frames: stream.engine_block_frames,
        });
        self.stream = Some(stream);
    }

    pub fn note_error(&mut self, error: impl Into<String>) {
        self.errors.push(error.into());
    }
}

pub fn finalize(config: &BenchmarkConfig, state: &mut RunState) -> Result<(), String> {
    if state.stream_started {
        let disabled_generation = state.phase_control.request(MeasurementPhase::Disabled);
        match state.phase_control.wait_for_ack(
            disabled_generation,
            MeasurementPhase::Disabled,
            Duration::from_secs(5),
        ) {
            Ok(_) => state.measurement_stop_acknowledged = true,
            Err(error) => state.note_error(error),
        }
        state.metrics.disable_measurement();
        if state.measurement_stop_acknowledged && state.profile_end.is_none() {
            match request_profile_snapshot(state) {
                Ok(snapshot) => state.profile_end = Some(snapshot),
                Err(error) => state.note_error(error),
            }
        }
        if state.stream.take().is_some() {
            state.stream_stopped = true;
            thread::sleep(SHUTDOWN_MARGIN);
        } else {
            state.note_error("benchmark stream was already absent during finalization");
        }
    }

    let final_metrics = state.metrics.snapshot();
    if final_metrics.terminal_error {
        state.note_error("callback reported a terminal CPAL, heartbeat, or geometry error");
    }
    if let Some(snapshot) = state.profile_start.as_ref() {
        if let Err(error) = validate_profile_state(snapshot, state.expected) {
            state.note_error(error);
        }
    } else if state.stream_started {
        state.note_error("initial profile evidence is missing");
    }
    if let Some(snapshot) = state.profile_end.as_ref() {
        if let Err(error) = validate_profile_state(snapshot, state.expected) {
            state.note_error(error);
        }
    } else if state.stream_started {
        state.note_error("final profile evidence is missing");
    }

    let worker_evidence = resolve_worker_evidence(config, state);
    if let Some(error) = worker_evidence.terminal_error.as_ref() {
        state.note_error(error.clone());
    }
    let worker_delta = worker_evidence.worker_delta;
    let worker_policy_error = worker_evidence.policy_error;

    let final_progress_result = write_final_progress(config, state, &final_metrics);
    let final_progress_write_succeeded = final_progress_result.is_ok();
    if let Err(error) = final_progress_result {
        state.note_error(error);
    }

    let status = result_status(
        config,
        &final_metrics,
        FinalizationGates {
            no_terminal_errors: state.errors.is_empty(),
            scheduler_qualified: state.scheduler_qualified,
            measurement_stop_acknowledged: state.measurement_stop_acknowledged,
            stream_stopped: state.stream_stopped,
            final_progress_write_succeeded,
        },
        worker_delta.as_ref(),
        worker_policy_error.as_deref(),
    );
    let stream = state.stream_evidence.clone().unwrap_or(StreamEvidence {
        sample_format: "unknown".into(),
        channels: 0,
        sample_rate: realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE,
        workers_effective: false,
        engine_block_frames: config.internal_frames,
    });
    let profile_start = state
        .profile_start
        .map(BenchmarkProfileSnapshot::from)
        .unwrap_or_default();
    let profile_end = state
        .profile_end
        .map(BenchmarkProfileSnapshot::from)
        .unwrap_or_default();
    let result = BenchmarkResult {
        schema_version: 3,
        kind: "orange_audio_benchmark_result".into(),
        status: status.into(),
        board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
        scenario: config.scenario.as_str().into(),
        requested_output_buffer_frames: config.output_frames,
        expected_alsa_buffer_frames: config.output_frames,
        expected_alsa_period_frames: config.expected_alsa_period_frames,
        internal_block_frames: stream.engine_block_frames,
        sample_format: stream.sample_format,
        channels: stream.channels,
        sample_rate: stream.sample_rate,
        workers_requested: config.workers,
        workers_effective: stream.workers_effective,
        warmup_seconds: config.warmup_seconds,
        measure_seconds: config.measure_seconds,
        scheduler_qualified: state.scheduler_qualified,
        post_dsp_zero: final_metrics.callback_count > 0
            && final_metrics.post_mute_nonzero_samples == 0,
        measurement_stop_acknowledged: state.measurement_stop_acknowledged,
        stream_stopped: state.stream_stopped,
        final_progress_write_succeeded,
        pid: std::process::id(),
        systemd_invocation_id: state.invocation_id.clone(),
        artifact_sha256: config.artifact_sha256.clone(),
        callback: final_metrics,
        profile_start,
        profile_end,
        worker_delta,
        worker_policy_error,
        recovered_alsa_epipe_count: None,
        recovered_alsa_epipe_observable: false,
        terminal_error: (!state.errors.is_empty()).then(|| state.errors.join("; ")),
    };

    atomic_write_json(&config.result_path, &result)
        .map_err(|error| format!("failed to write the single terminal result: {error}"))?;
    if status == "pass" {
        Ok(())
    } else {
        Err("Orange benchmark terminal result failed its evidence gates".into())
    }
}

pub fn request_profile_snapshot(state: &RunState) -> Result<SynthProfileSnapshot, String> {
    let generation = state.profile_probe.request();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Some(snapshot) = state.profile_probe.poll(generation) {
            return Ok(snapshot);
        }
        if state.metrics.snapshot().terminal_error {
            return Err("callback error occurred while waiting for profile snapshot".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err("profile snapshot probe timed out".into())
}

pub fn validate_profile_state(
    snapshot: &SynthProfileSnapshot,
    expected: ExpectedLiveState,
) -> Result<(), String> {
    let actual = (
        snapshot.active_synth_voices,
        snapshot.active_sample_voices,
        snapshot.active_momentary_fx,
        snapshot.cumulative_voice_steals,
    );
    let expected = (
        expected.active_synth_voices,
        expected.active_sample_voices,
        expected.active_momentary_fx,
        expected.expected_voice_steals,
    );
    if actual != expected {
        return Err(format!(
            "fixture state mismatch: actual={actual:?} expected={expected:?}"
        ));
    }
    Ok(())
}

fn validate_workers(
    config: &BenchmarkConfig,
    state: &RunState,
) -> Result<(BenchmarkWorkerDelta, Option<String>), String> {
    let Some(start) = state.profile_start.as_ref() else {
        return Err("worker start profile evidence is missing".into());
    };
    let Some(end) = state.profile_end.as_ref() else {
        return Err("worker end profile evidence is missing".into());
    };
    let Some(stream) = state.stream_evidence.as_ref() else {
        return Err("worker stream evidence is missing".into());
    };
    let start = BenchmarkProfileSnapshot::from(*start);
    let end = BenchmarkProfileSnapshot::from(*end);
    let policy = worker::policy(
        config.internal_frames,
        config.workers,
        config.scenario.as_str(),
    );
    let delta = worker::delta(&start, &end)?;
    worker::validate_configuration(policy, stream.workers_effective, &start, &end)?;
    let policy_error = worker::validate_policy(policy, &delta, config.scenario.as_str()).err();
    Ok((delta, policy_error))
}

#[derive(Debug, PartialEq, Eq)]
struct WorkerEvidence {
    worker_delta: Option<BenchmarkWorkerDelta>,
    policy_error: Option<String>,
    terminal_error: Option<String>,
}

fn resolve_worker_evidence(config: &BenchmarkConfig, state: &RunState) -> WorkerEvidence {
    match validate_workers(config, state) {
        Ok((delta, policy_error)) => WorkerEvidence {
            worker_delta: Some(delta),
            policy_error,
            terminal_error: None,
        },
        Err(error) => WorkerEvidence {
            worker_delta: None,
            policy_error: None,
            terminal_error: Some(error),
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FinalizationGates {
    no_terminal_errors: bool,
    scheduler_qualified: bool,
    measurement_stop_acknowledged: bool,
    stream_stopped: bool,
    final_progress_write_succeeded: bool,
}

fn result_status(
    config: &BenchmarkConfig,
    metrics: &CallbackMetricsSnapshot,
    gates: FinalizationGates,
    worker_delta: Option<&BenchmarkWorkerDelta>,
    worker_policy_error: Option<&str>,
) -> &'static str {
    if gates.no_terminal_errors
        && gates.scheduler_qualified
        && gates.measurement_stop_acknowledged
        && gates.stream_stopped
        && gates.final_progress_write_succeeded
        && worker_policy_error.is_none()
        && worker_delta.is_some_and(|delta| result_passes(config, metrics, delta))
    {
        "pass"
    } else {
        "fail"
    }
}

fn result_passes(
    config: &BenchmarkConfig,
    metrics: &CallbackMetricsSnapshot,
    workers: &BenchmarkWorkerDelta,
) -> bool {
    metrics.callback_count > 0
        && metrics.callback_frames_min > 0
        && metrics.callback_frames_max <= config.output_frames
        && metrics.callback_frame_sample_count == metrics.callback_count
        && metrics.invalid_callback_frame_count == 0
        && metrics.over_audio_duration_budget_count == 0
        && metrics.pre_mute_nonzero_samples > 0
        && metrics.post_mute_nonzero_samples == 0
        && !metrics.terminal_error
        && workers.synth_parallel_light_skips == 0
        && workers.synth_parallel_backoff_skips == 0
        && workers.synth_parallel_timing_backoffs == 0
        && workers.synth_parallel_failures == 0
        && !workers.synth_parallel_unhealthy
}

fn write_final_progress(
    config: &BenchmarkConfig,
    state: &RunState,
    metrics: &CallbackMetricsSnapshot,
) -> Result<(), String> {
    atomic_write_json(
        &config.progress_path,
        &BenchmarkProgress::new(
            config,
            "finalizing",
            metrics.measured_elapsed_ns / 1_000_000_000,
            config.measure_seconds,
            metrics,
            state
                .stream_evidence
                .as_ref()
                .map(|stream| stream.workers_effective),
        ),
    )
}

#[cfg(test)]
#[path = "finalization_tests.rs"]
mod tests;
