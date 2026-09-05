use super::cli::{BenchmarkConfig, BenchmarkExecutorMode, RecordedGeometry, WorkerTimingMode};
use super::metrics::{CallbackMetrics, CallbackMetricsSnapshot};
use super::phase::{MeasurementControl, MeasurementPhase};
use super::probe::ProfileProbe;
use super::schema::{
    atomic_write_json, BenchmarkProfileSnapshot, BenchmarkProgress, BenchmarkResult,
    BenchmarkWorkerTiming, PersistentOutputCountersEvidence,
};
use super::stream::BenchmarkStream;
use crate::audio::AudioStreamShutdownError;
use crate::audio_priority::EffectiveScheduling;
use crate::dsp_scenarios::ExpectedLiveState;
use realtime_engine::synth::{SourceWorkerHealth, SourceWorkerTimingProbe, SynthProfileSnapshot};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[path = "finalization_profile_validation.rs"]
mod profile_validation;
pub(super) use profile_validation::validate_profile_state;

const SHUTDOWN_MARGIN: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
pub struct StreamEvidence {
    pub sample_format: String,
    pub channels: u16,
    pub sample_rate: u32,
    pub engine_block_frames: usize,
    pub lookahead_frames: usize,
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
    pub worker_health: SourceWorkerHealth,
    pub worker_thread_names: [String; 2],
    pub joined_workers: usize,
    pub retirement_error: Option<String>,
    pub callback_scheduling: Option<EffectiveScheduling>,
    pub worker_timing: Option<Arc<SourceWorkerTimingProbe>>,
    pub persistent_output_counters: PersistentOutputCountersEvidence,
}

impl RunState {
    pub fn new(
        expected: ExpectedLiveState,
        sample_rate: u32,
        expected_period_frames: u32,
        max_callback_frames: u32,
        executor_mode: BenchmarkExecutorMode,
        worker_timing_mode: WorkerTimingMode,
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
            worker_health: SourceWorkerHealth::Disabled,
            worker_thread_names: [String::new(), String::new()],
            joined_workers: 0,
            retirement_error: None,
            callback_scheduling: None,
            worker_timing: (worker_timing_mode == WorkerTimingMode::Enabled).then(|| {
                Arc::new(SourceWorkerTimingProbe::new(Some(
                    crate::audio_priority::orange_cpu_sampler,
                )))
            }),
            persistent_output_counters: PersistentOutputCountersEvidence::for_executor(
                executor_mode,
            ),
        }
    }

    pub fn install_stream(&mut self, stream: BenchmarkStream) {
        self.stream_evidence = Some(StreamEvidence {
            sample_format: stream.sample_format.clone(),
            channels: stream.channels,
            sample_rate: stream.sample_rate,
            engine_block_frames: stream.engine_block_frames,
            lookahead_frames: stream.lookahead_frames,
        });
        self.worker_health = stream.worker_health();
        self.worker_thread_names = stream.worker_thread_names();
        self.stream = Some(stream);
    }

    pub fn current_worker_health(&self) -> SourceWorkerHealth {
        self.stream
            .as_ref()
            .map(BenchmarkStream::worker_health)
            .unwrap_or(self.worker_health)
    }

    pub fn note_error(&mut self, error: impl Into<String>) {
        self.errors.push(error.into());
    }

    pub fn phase_boundary_counters(
        &self,
        generation: u64,
    ) -> Result<rodio_engine_source::PersistentOutputCounters, String> {
        self.metrics
            .phase_boundary_snapshot(generation)
            .ok_or_else(|| {
                format!("phase boundary counters are missing for generation {generation}")
            })
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
            Ok(capture) => {
                state.measurement_stop_acknowledged = true;
                match state.phase_boundary_counters(capture.generation) {
                    Ok(counters) => state.persistent_output_counters.set_end(counters),
                    Err(error) => state.note_error(error),
                }
            }
            Err(error) => state.note_error(error),
        }
        state.metrics.disable_measurement();
        if state.measurement_stop_acknowledged && state.profile_end.is_none() {
            match request_profile_snapshot(state) {
                Ok(snapshot) => state.profile_end = Some(snapshot),
                Err(error) => state.note_error(error),
            }
        }
        if let Some(stream) = state.stream.take() {
            state.worker_health = stream.worker_health();
            state.worker_thread_names = stream.worker_thread_names();
            stream.report_worker_terminal();
            state.stream_stopped = true;
            match stream.teardown() {
                Ok(report) => {
                    state.joined_workers = report.joined_workers;
                    state.retirement_error =
                        report.retirement_error.map(|error| format!("{error:?}"));
                }
                Err(error) => record_shutdown_error(state, error),
            }
            thread::sleep(SHUTDOWN_MARGIN);
        } else {
            state.note_error("benchmark stream was already absent during finalization");
        }
    }

    if let Err(error) = state.persistent_output_counters.calculate_delta() {
        state.note_error(error);
    }
    let final_metrics = state.metrics.snapshot();
    if final_metrics.terminal_error {
        state.note_error("callback reported a terminal CPAL, heartbeat, or geometry error");
    }
    if let Some(snapshot) = state.profile_start.as_ref() {
        if let Err(error) = validate_profile_state(
            snapshot,
            state.expected,
            state.expected.expected_voice_admission_drops_start,
        ) {
            state.note_error(error);
        }
    } else if state.stream_started {
        state.note_error("initial profile evidence is missing");
    }
    if let Some(snapshot) = state.profile_end.as_ref() {
        if let Err(error) = validate_profile_state(
            snapshot,
            state.expected,
            state.expected.expected_voice_admission_drops_end,
        ) {
            state.note_error(error);
        }
    } else if state.stream_started {
        state.note_error("final profile evidence is missing");
    }

    let final_progress_result = write_final_progress(config, &final_metrics, state.worker_health);
    let final_progress_write_succeeded = final_progress_result.is_ok();
    if let Err(error) = final_progress_result {
        state.note_error(error);
    }

    let detected_continuity_events = state
        .persistent_output_counters
        .detected_continuity_events(final_metrics.over_audio_duration_budget_count);
    let stream = state.stream_evidence.clone().unwrap_or(StreamEvidence {
        sample_format: "unknown".into(),
        channels: 0,
        sample_rate: realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE,
        engine_block_frames: config.internal_frames,
        lookahead_frames: super::cli::expected_lookahead_frames(
            config.executor_mode,
            config.internal_frames,
        ),
    });
    if let Err(error) = super::cli::validate_recorded_geometry(RecordedGeometry {
        scenario: config.scenario.as_str(),
        executor_mode: config.executor_mode,
        requested_output_buffer_frames: config.output_frames,
        expected_alsa_buffer_frames: config.output_frames,
        expected_alsa_period_frames: config.expected_alsa_period_frames,
        internal_block_frames: stream.engine_block_frames,
        lookahead_frames: stream.lookahead_frames,
        effective_output_latency_frames: Some(
            config.output_frames as usize + stream.lookahead_frames,
        ),
    }) {
        state.note_error(error);
    }
    let status = result_status(
        config,
        &final_metrics,
        detected_continuity_events,
        FinalizationGates {
            no_terminal_errors: state.errors.is_empty(),
            scheduler_qualified: state.scheduler_qualified,
            measurement_stop_acknowledged: state.measurement_stop_acknowledged,
            stream_stopped: state.stream_stopped,
            final_progress_write_succeeded,
            worker_health: state.worker_health,
            worker_thread_names: state.worker_thread_names.clone(),
            joined_workers: state.joined_workers,
            retirement_error: state.retirement_error.is_none(),
            worker_timing_consistent: worker_timing_is_consistent(config, state),
        },
    );
    let profile_start = state
        .profile_start
        .map(BenchmarkProfileSnapshot::from)
        .unwrap_or_default();
    let profile_end = state
        .profile_end
        .map(BenchmarkProfileSnapshot::from)
        .unwrap_or_default();
    let result = BenchmarkResult {
        schema_version: 12,
        kind: "orange_audio_benchmark_result".into(),
        status: status.into(),
        board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
        scenario: config.scenario.as_str().into(),
        requested_output_buffer_frames: config.output_frames,
        expected_alsa_buffer_frames: config.output_frames,
        expected_alsa_period_frames: config.expected_alsa_period_frames,
        internal_block_frames: stream.engine_block_frames,
        lookahead_frames: stream.lookahead_frames,
        effective_output_latency_frames: config.output_frames as usize + stream.lookahead_frames,
        sample_format: stream.sample_format,
        channels: stream.channels,
        sample_rate: stream.sample_rate,
        warmup_seconds: config.warmup_seconds,
        measure_seconds: config.measure_seconds,
        scheduler_qualified: state.scheduler_qualified,
        callback_scheduling_policy: state.callback_scheduling.map(|scheduling| {
            crate::audio_priority::scheduling_policy_name(scheduling.policy).into()
        }),
        callback_scheduling_priority: state
            .callback_scheduling
            .map(|scheduling| scheduling.priority),
        callback_scheduling_cpu: state
            .callback_scheduling
            .and_then(|scheduling| scheduling.cpu)
            .map(|cpu| cpu as u32),
        post_dsp_zero: final_metrics.callback_count > 0
            && final_metrics.post_mute_nonzero_samples == 0,
        measurement_stop_acknowledged: state.measurement_stop_acknowledged,
        stream_stopped: state.stream_stopped,
        final_progress_write_succeeded,
        pid: std::process::id(),
        systemd_invocation_id: state.invocation_id.clone(),
        artifact_sha256: config.artifact_sha256.clone(),
        callback: final_metrics,
        persistent_output_counters: state.persistent_output_counters,
        detected_continuity_events,
        profile_start,
        profile_end,
        recovered_alsa_epipe_count: None,
        recovered_alsa_epipe_observable: false,
        terminal_error: (!state.errors.is_empty()).then(|| state.errors.join("; ")),
        executor_mode: config.executor_mode.as_str().into(),
        worker_timing_mode: config.worker_timing_mode,
        worker_health: state.worker_health.name().into(),
        worker_thread_name_0: state.worker_thread_names[0].clone(),
        worker_thread_name_1: state.worker_thread_names[1].clone(),
        joined_workers: state.joined_workers,
        retirement_error: state.retirement_error.clone(),
        worker_timing: state.worker_timing.as_ref().map(|probe| {
            probe.freeze_latest_completed();
            BenchmarkWorkerTiming::from(probe.snapshot())
        }),
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
        if let Some(stream) = state.stream.as_ref() {
            if stream.runtime_status() == crate::audio::AudioStreamStatus::Terminal {
                stream.report_worker_terminal();
                return Err("benchmark DSP worker entered a terminal health state".into());
            }
        }
        if state.metrics.snapshot().terminal_error {
            return Err("callback error occurred while waiting for profile snapshot".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err("profile snapshot probe timed out".into())
}

fn record_shutdown_error(state: &mut RunState, error: AudioStreamShutdownError) {
    match error {
        AudioStreamShutdownError::WorkerStatus {
            joined_workers,
            retirement_error,
        } => {
            state.joined_workers = joined_workers;
            state.retirement_error = retirement_error.map(|error| format!("{error:?}"));
        }
        AudioStreamShutdownError::Retirement(error) => {
            state.retirement_error = Some(format!("{error:?}"));
        }
        AudioStreamShutdownError::ReaperCompletionUnavailable
        | AudioStreamShutdownError::ReaperThreadPanicked => {}
    }
    state.note_error(format!("benchmark stream teardown failed: {error:?}"));
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FinalizationGates {
    no_terminal_errors: bool,
    scheduler_qualified: bool,
    measurement_stop_acknowledged: bool,
    stream_stopped: bool,
    final_progress_write_succeeded: bool,
    worker_health: SourceWorkerHealth,
    worker_thread_names: [String; 2],
    joined_workers: usize,
    retirement_error: bool,
    worker_timing_consistent: bool,
}

fn result_status(
    config: &BenchmarkConfig,
    metrics: &CallbackMetricsSnapshot,
    detected_continuity_events: u64,
    gates: FinalizationGates,
) -> &'static str {
    if gates.no_terminal_errors
        && gates.scheduler_qualified
        && gates.measurement_stop_acknowledged
        && gates.stream_stopped
        && gates.final_progress_write_succeeded
        && worker_lifecycle_passes(config, &gates)
        && worker_timing_passes(config, &gates)
        && gates.retirement_error
        && result_passes(config, metrics, detected_continuity_events)
    {
        "pass"
    } else {
        "fail"
    }
}

fn worker_lifecycle_passes(config: &BenchmarkConfig, gates: &FinalizationGates) -> bool {
    match config.executor_mode {
        super::cli::BenchmarkExecutorMode::Inline => {
            gates.worker_health == SourceWorkerHealth::Disabled
                && gates.worker_thread_names == [String::new(), String::new()]
                && gates.joined_workers == 0
        }
        super::cli::BenchmarkExecutorMode::PersistentTwoWorkers
        | super::cli::BenchmarkExecutorMode::RoutingTreePersistent => {
            gates.worker_health == SourceWorkerHealth::Healthy
                && gates.worker_thread_names
                    == super::stream::worker_thread_names_for_executor(config.executor_mode)
                && gates.joined_workers == 2
        }
    }
}

fn worker_timing_passes(config: &BenchmarkConfig, gates: &FinalizationGates) -> bool {
    gates.worker_timing_consistent
        && (config.executor_mode != super::cli::BenchmarkExecutorMode::Inline
            || config.worker_timing_mode == WorkerTimingMode::Disabled)
}

fn worker_timing_is_consistent(config: &BenchmarkConfig, state: &RunState) -> bool {
    (config.executor_mode != super::cli::BenchmarkExecutorMode::Inline
        || config.worker_timing_mode == WorkerTimingMode::Disabled)
        && state.worker_timing.is_some() == (config.worker_timing_mode == WorkerTimingMode::Enabled)
}

fn result_passes(
    config: &BenchmarkConfig,
    metrics: &CallbackMetricsSnapshot,
    detected_continuity_events: u64,
) -> bool {
    metrics.callback_count > 0
        && metrics.callback_frames_min > 0
        && metrics.callback_frames_max <= config.output_frames
        && metrics.callback_frame_sample_count == metrics.callback_count
        && metrics.invalid_callback_frame_count == 0
        && callback_budget_passes(
            config.measure_seconds,
            metrics.over_audio_duration_budget_count,
        )
        && (config.measure_seconds != 180
            || (detected_continuity_events == 0
                && metrics.over_audio_duration_budget_count == 0
                && metrics.cpal_device_error_count == 0
                && metrics.cpal_stream_error_count == 0))
        && metrics.pre_mute_nonzero_samples > 0
        && metrics.post_mute_nonzero_samples == 0
        && !metrics.worker_terminal
        && !metrics.terminal_error
}

fn callback_budget_passes(measure_seconds: u64, overrun_count: u64) -> bool {
    match measure_seconds {
        30 | 120 => overrun_count == 0,
        180 => overrun_count == 0,
        300 => overrun_count <= 5,
        _ => false,
    }
}

fn write_final_progress(
    config: &BenchmarkConfig,
    metrics: &CallbackMetricsSnapshot,
    worker_health: SourceWorkerHealth,
) -> Result<(), String> {
    atomic_write_json(
        &config.progress_path,
        &BenchmarkProgress::new(
            config,
            "finalizing",
            metrics.measured_elapsed_ns / 1_000_000_000,
            config.measure_seconds,
            metrics,
            worker_health,
        ),
    )
}

#[cfg(test)]
#[path = "finalization_tests.rs"]
mod finalization_tests;
