use super::cli::{BenchmarkConfig, BenchmarkExecutorMode, RecordedGeometry};
use super::metrics::CallbackMetrics;
use super::phase::MeasurementControl;
use super::probe::ProfileProbe;
use crate::audio::{
    AudioStreamBuildError, AudioStreamHealth, AudioStreamLifecycle, AudioStreamShutdownError,
    AudioStreamShutdownReport, CallbackSource,
};
use crate::audio_priority::CallbackSchedulingHandle;
use crate::orange_audio::select_orange_stream_config;
use cpal::{BufferSize, SampleFormat, Stream};
use realtime_engine::synth::{
    SourceWorkerHealth, SourceWorkerTimingProbe, ROUTING_TREE_WORKER_THREAD_NAMES,
    SOURCE_WORKER_THREAD_NAMES,
};
use rodio_engine_source::{EngineEventReceiver, EngineSource, EngineSourceWorkerShutdownOwner};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

#[path = "stream_callback.rs"]
mod callback;

#[cfg(test)]
pub const EXECUTOR_MODE: &str = "routing_tree_persistent";

pub struct BenchmarkStream {
    lifecycle: AudioStreamLifecycle<Stream, EngineSourceWorkerShutdownOwner>,
    pub scheduler: CallbackSchedulingHandle,
    pub sample_format: String,
    pub channels: u16,
    pub sample_rate: u32,
    pub engine_block_frames: usize,
    pub lookahead_frames: usize,
    health: AudioStreamHealth,
    worker_health: Arc<AtomicU8>,
    worker_thread_names: [String; 2],
}

impl BenchmarkStream {
    pub fn play(&self) -> Result<(), cpal::PlayStreamError> {
        self.lifecycle.play()
    }

    pub fn teardown(self) -> Result<AudioStreamShutdownReport, AudioStreamShutdownError> {
        self.lifecycle.teardown()
    }

    pub fn runtime_status(&self) -> crate::audio::AudioStreamStatus {
        self.health.runtime_status()
    }

    pub fn health(&self) -> &AudioStreamHealth {
        &self.health
    }

    pub fn report_worker_terminal(&self) {
        self.health.log_worker_terminal_once();
    }

    pub fn worker_health(&self) -> SourceWorkerHealth {
        SourceWorkerHealth::from_u8(self.worker_health.load(Ordering::Acquire))
    }

    pub fn worker_thread_names(&self) -> [String; 2] {
        self.worker_thread_names.clone()
    }
}

pub fn expected_worker_thread_names() -> [String; 2] {
    SOURCE_WORKER_THREAD_NAMES.map(str::to_owned)
}

pub fn expected_routing_worker_thread_names() -> [String; 2] {
    ROUTING_TREE_WORKER_THREAD_NAMES.map(str::to_owned)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StreamGeometry {
    output_frames: u32,
    internal_frames: usize,
}

pub fn build(
    engine_rx: EngineEventReceiver,
    config: &BenchmarkConfig,
    metrics: Arc<CallbackMetrics>,
    profile_probe: Arc<ProfileProbe>,
    phase_control: Arc<MeasurementControl>,
    timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
) -> Result<BenchmarkStream, String> {
    super::cli::preflight(config)?;
    let executor_mode = config.executor_mode;
    let output_frames = config.output_frames;
    let internal_frames = config.internal_frames;
    if executor_mode == BenchmarkExecutorMode::Inline && timing_probe.is_some() {
        return Err("inline executor requires disabled worker timing".into());
    }
    let geometry = stream_geometry(output_frames, internal_frames)?;
    let device =
        crate::orange_audio::select_orange_output_device().map_err(|error| error.to_string())?;
    let (sample_format, mut stream_config) =
        select_orange_stream_config(&device).map_err(|error| error.to_string())?;
    let sample_format_name = format!("{sample_format:?}");
    debug_assert_eq!(geometry.output_frames, output_frames);
    stream_config.buffer_size = BufferSize::Fixed(output_frames);
    let (source, shutdown_owner) = build_source(
        engine_rx,
        executor_mode,
        stream_config.sample_rate.0,
        geometry.internal_frames,
        timing_probe.clone(),
    )?;
    let engine_block_frames = source.block_frames();
    let lookahead_frames = source.lookahead_frames();
    super::cli::validate_recorded_geometry(RecordedGeometry {
        scenario: config.scenario.as_str(),
        executor_mode,
        requested_output_buffer_frames: output_frames,
        expected_alsa_buffer_frames: output_frames,
        expected_alsa_period_frames: config.expected_alsa_period_frames,
        internal_block_frames: engine_block_frames,
        lookahead_frames,
        effective_output_latency_frames: None,
    })?;
    let health = AudioStreamHealth::new("Orange benchmark".into());
    let worker_health = Arc::new(AtomicU8::new(source.source_worker_health() as u8));
    let (callback_source, retirement_waiter) = CallbackSource::new(source, true);
    let scheduler = callback_scheduler_for_executor(executor_mode);
    let callback_scheduler = scheduler.clone();
    let callback_context = callback::CallbackContext {
        metrics,
        profile_probe,
        phase_control,
        health: health.clone(),
        worker_health: worker_health.clone(),
        timing_probe,
    };
    let stream_result = match sample_format {
        SampleFormat::F32 => callback::build_typed::<f32>(
            &device,
            &stream_config,
            callback_source,
            callback_scheduler,
            callback_context,
        ),
        SampleFormat::I16 => callback::build_typed::<i16>(
            &device,
            &stream_config,
            callback_source,
            callback_scheduler,
            callback_context,
        ),
        SampleFormat::U16 => callback::build_typed::<u16>(
            &device,
            &stream_config,
            callback_source,
            callback_scheduler,
            callback_context,
        ),
        format => {
            drop(callback_source);
            Err(format!(
                "unsupported Orange benchmark sample format: {format:?}"
            ))
        }
    };
    let lifecycle =
        AudioStreamLifecycle::from_build_result(stream_result, shutdown_owner, retirement_waiter)
            .map_err(map_build_error)?;
    Ok(BenchmarkStream {
        lifecycle,
        scheduler,
        sample_format: sample_format_name,
        channels: stream_config.channels,
        sample_rate: stream_config.sample_rate.0,
        engine_block_frames,
        lookahead_frames,
        health,
        worker_health,
        worker_thread_names: worker_thread_names_for_executor(executor_mode),
    })
}

fn build_source(
    engine_rx: EngineEventReceiver,
    executor_mode: BenchmarkExecutorMode,
    sample_rate: u32,
    internal_frames: usize,
    timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
) -> Result<(EngineSource, Option<EngineSourceWorkerShutdownOwner>), String> {
    if executor_mode == BenchmarkExecutorMode::Inline {
        return Ok((
            EngineSource::with_block_frames(engine_rx, sample_rate, internal_frames),
            None,
        ));
    }
    match executor_mode {
        BenchmarkExecutorMode::PersistentTwoWorkers => {
            Err("persistent_two_workers executor was removed; use routing_tree_persistent".into())
        }
        BenchmarkExecutorMode::RoutingTreePersistent => {
            let (source, shutdown_owner) =
                build_routing_tree_source(engine_rx, sample_rate, internal_frames, timing_probe)?;
            Ok((source, Some(shutdown_owner)))
        }
        BenchmarkExecutorMode::Inline => unreachable!("inline source handled above"),
    }
}

#[cfg(feature = "routing-tree-benchmark")]
fn build_routing_tree_source(
    engine_rx: EngineEventReceiver,
    sample_rate: u32,
    internal_frames: usize,
    timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
) -> Result<(EngineSource, EngineSourceWorkerShutdownOwner), String> {
    let result = match timing_probe {
        Some(timing_probe) => {
            EngineSource::with_routing_tree_persistent_workers_for_benchmark_with_timing_probe_and_hook(
                engine_rx,
                sample_rate,
                internal_frames,
                None,
                timing_probe,
                crate::audio_priority::orange_worker_start_hook,
            )
        }
        None => EngineSource::with_routing_tree_persistent_workers_with_hook(
            engine_rx,
            sample_rate,
            internal_frames,
            None,
            crate::audio_priority::orange_worker_start_hook,
        ),
    };
    result.map_err(|error| {
        format!("failed to start routing-tree Orange benchmark workers: {error:?}")
    })
}

#[cfg(not(feature = "routing-tree-benchmark"))]
fn build_routing_tree_source(
    _engine_rx: EngineEventReceiver,
    _sample_rate: u32,
    _internal_frames: usize,
    _timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
) -> Result<(EngineSource, EngineSourceWorkerShutdownOwner), String> {
    Err(super::cli::ROUTING_TREE_FEATURE_REQUIRED_ERROR.into())
}

fn callback_scheduler_for_executor(
    _executor_mode: BenchmarkExecutorMode,
) -> CallbackSchedulingHandle {
    CallbackSchedulingHandle::new_orange_jack()
}

pub(super) fn worker_thread_names_for_executor(
    executor_mode: BenchmarkExecutorMode,
) -> [String; 2] {
    match executor_mode {
        BenchmarkExecutorMode::Inline => [String::new(), String::new()],
        BenchmarkExecutorMode::PersistentTwoWorkers => expected_worker_thread_names(),
        BenchmarkExecutorMode::RoutingTreePersistent => expected_routing_worker_thread_names(),
    }
}

fn map_build_error(error: AudioStreamBuildError<String>) -> String {
    match error {
        AudioStreamBuildError::Stream(error) => error,
        AudioStreamBuildError::Shutdown(error) => {
            format!("Orange benchmark teardown failed during build: {error:?}")
        }
    }
}

fn stream_geometry(output_frames: u32, internal_frames: usize) -> Result<StreamGeometry, String> {
    if !matches!(
        (output_frames, internal_frames),
        (128, 32) | (128, 64) | (256, 64) | (256, 128) | (256, 256) | (512, 128) | (1024, 256)
    ) {
        return Err(format!(
            "benchmark output/internal frame mapping is invalid: output={output_frames} internal={internal_frames}"
        ));
    }
    Ok(StreamGeometry {
        output_frames,
        internal_frames,
    })
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
