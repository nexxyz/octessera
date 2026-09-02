use super::metrics::{CallbackMetrics, CallbackPrefix};
use super::phase::{same_measuring_generation, MeasurementControl, MeasurementPhase, PhaseCapture};
use super::probe::ProfileProbe;
use crate::audio::{
    AudioStreamBuildError, AudioStreamHealth, AudioStreamLifecycle, AudioStreamShutdownError,
    AudioStreamShutdownReport, CallbackSource,
};
use crate::audio_priority::CallbackSchedulingHandle;
use crate::orange_audio::select_orange_stream_config;
use cpal::traits::DeviceTrait;
use cpal::{BufferSize, SampleFormat, Stream, StreamConfig};
use realtime_engine::synth::{SourceWorkerHealth, SOURCE_WORKER_THREAD_NAMES};
use rodio_engine_source::{EngineEventReceiver, EngineSource, EngineSourceWorkerShutdownOwner};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub const EXECUTOR_MODE: &str = "persistent_two_workers";

pub struct BenchmarkStream {
    lifecycle: AudioStreamLifecycle<Stream, EngineSourceWorkerShutdownOwner>,
    pub scheduler: CallbackSchedulingHandle,
    pub sample_format: String,
    pub channels: u16,
    pub sample_rate: u32,
    pub engine_block_frames: usize,
    health: AudioStreamHealth,
    worker_health: Arc<AtomicU8>,
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
        source_worker_health_from_u8(self.worker_health.load(Ordering::Acquire))
    }

    pub fn worker_thread_names(&self) -> [String; 2] {
        expected_worker_thread_names()
    }
}

pub fn expected_worker_thread_names() -> [String; 2] {
    SOURCE_WORKER_THREAD_NAMES.map(str::to_owned)
}

pub fn source_worker_health_name(health: SourceWorkerHealth) -> &'static str {
    match health {
        SourceWorkerHealth::Disabled => "disabled",
        SourceWorkerHealth::Healthy => "healthy",
        SourceWorkerHealth::DeadlineMiss => "deadline_miss",
        SourceWorkerHealth::DispatchFailed => "dispatch_failed",
        SourceWorkerHealth::CompletionFailed => "completion_failed",
        SourceWorkerHealth::WorkerExited => "worker_exited",
        SourceWorkerHealth::InvalidBlock => "invalid_block",
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StreamGeometry {
    output_frames: u32,
    internal_frames: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CallbackBodyStats {
    pre_mute_nonzero: u64,
    pre_mute_peak: f32,
    post_mute_nonzero: u64,
}

struct CallbackContext {
    metrics: Arc<CallbackMetrics>,
    profile_probe: Arc<ProfileProbe>,
    phase_control: Arc<MeasurementControl>,
    health: AudioStreamHealth,
    worker_health: Arc<AtomicU8>,
}

pub fn build(
    engine_rx: EngineEventReceiver,
    output_frames: u32,
    internal_frames: usize,
    metrics: Arc<CallbackMetrics>,
    profile_probe: Arc<ProfileProbe>,
    phase_control: Arc<MeasurementControl>,
) -> Result<BenchmarkStream, String> {
    let geometry = stream_geometry(output_frames, internal_frames)?;
    let device =
        crate::orange_audio::select_orange_output_device().map_err(|error| error.to_string())?;
    let (sample_format, mut config) =
        select_orange_stream_config(&device).map_err(|error| error.to_string())?;
    let sample_format_name = format!("{sample_format:?}");
    debug_assert_eq!(geometry.output_frames, output_frames);
    config.buffer_size = BufferSize::Fixed(output_frames);
    let (source, shutdown_owner) =
        build_persistent_source(engine_rx, config.sample_rate.0, geometry.internal_frames)?;
    let engine_block_frames = source.block_frames();
    let health = AudioStreamHealth::new("Orange benchmark".into());
    let worker_health = Arc::new(AtomicU8::new(source.source_worker_health() as u8));
    let (callback_source, retirement_waiter) = CallbackSource::new(source, true);
    let scheduler = CallbackSchedulingHandle::new(crate::audio_priority::configured_priority());
    let callback_scheduler = scheduler.clone();
    let callback_context = CallbackContext {
        metrics,
        profile_probe,
        phase_control,
        health: health.clone(),
        worker_health: worker_health.clone(),
    };
    let stream_result = match sample_format {
        SampleFormat::F32 => build_typed::<f32>(
            &device,
            &config,
            callback_source,
            callback_scheduler,
            callback_context,
        ),
        SampleFormat::I16 => build_typed::<i16>(
            &device,
            &config,
            callback_source,
            callback_scheduler,
            callback_context,
        ),
        SampleFormat::U16 => build_typed::<u16>(
            &device,
            &config,
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
    let lifecycle = AudioStreamLifecycle::from_build_result(
        stream_result,
        Some(shutdown_owner),
        retirement_waiter,
    )
    .map_err(map_build_error)?;
    Ok(BenchmarkStream {
        lifecycle,
        scheduler,
        sample_format: sample_format_name,
        channels: config.channels,
        sample_rate: config.sample_rate.0,
        engine_block_frames,
        health,
        worker_health,
    })
}

fn build_persistent_source(
    engine_rx: EngineEventReceiver,
    sample_rate: u32,
    internal_frames: usize,
) -> Result<(EngineSource, EngineSourceWorkerShutdownOwner), String> {
    EngineSource::with_persistent_workers_for_benchmark(
        engine_rx,
        sample_rate,
        internal_frames,
        None,
    )
    .map_err(|error| format!("failed to start persistent Orange benchmark workers: {error:?}"))
}

fn map_build_error(error: AudioStreamBuildError<String>) -> String {
    match error {
        AudioStreamBuildError::Stream(error) => error,
        AudioStreamBuildError::Shutdown(error) => {
            format!("persistent Orange benchmark teardown failed during build: {error:?}")
        }
    }
}

fn stream_geometry(output_frames: u32, internal_frames: usize) -> Result<StreamGeometry, String> {
    if !matches!(
        (output_frames, internal_frames),
        (128, 32) | (256, 64) | (256, 128) | (256, 256) | (512, 128) | (1024, 256)
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

fn build_typed<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut callback_source: CallbackSource,
    callback_scheduler: CallbackSchedulingHandle,
    context: CallbackContext,
) -> Result<Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32> + PartialEq,
{
    let CallbackContext {
        metrics,
        profile_probe,
        phase_control,
        health: callback_health,
        worker_health,
    } = context;
    let mut previous_timestamp: Option<cpal::StreamInstant> = None;
    let mut previous_phase: Option<PhaseCapture> = None;
    let channels = config.channels;
    let callback_metrics = metrics.clone();
    let callback_health_for_error = callback_health.clone();
    let metrics_for_error = metrics.clone();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
                let body_started = Instant::now();
                let phase_capture = phase_control.capture_at_callback_entry();
                callback_scheduler.configure_callback_thread();
                let timestamp = info.timestamp().callback;
                let spacing = match (previous_timestamp, previous_phase) {
                    (Some(previous), Some(previous_phase))
                        if same_measuring_generation(previous_phase, phase_capture) =>
                    {
                        timestamp
                            .duration_since(&previous)
                            .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
                    }
                    _ => None,
                };
                previous_timestamp = Some(timestamp);
                previous_phase = Some(phase_capture);
                let body = fill_persistent_callback_body(
                    data,
                    &mut callback_source,
                    &callback_health,
                    &callback_metrics,
                    &worker_health,
                );
                let frames = (data.len() / usize::from(channels)) as u32;
                let measured = callback_metrics.record_prefix(CallbackPrefix {
                    entry_ns: phase_capture.entry_ns,
                    measured: phase_capture.phase == MeasurementPhase::Measuring,
                    frames,
                    pre_mute_nonzero: body.pre_mute_nonzero,
                    pre_mute_peak: body.pre_mute_peak,
                    post_mute_nonzero: body.post_mute_nonzero,
                    spacing_ns: spacing,
                });
                if profile_probe.request_pending() {
                    if let Some(source) = callback_source.source_mut() {
                        profile_probe.publish(source.profile_snapshot());
                    }
                }
                callback_metrics.publish_timing(measured, frames, body_started.elapsed());
            },
            move |error| {
                let is_device_error = matches!(&error, cpal::StreamError::DeviceNotAvailable);
                callback_health_for_error.log(error);
                if is_device_error {
                    metrics_for_error.record_cpal_device_error()
                } else {
                    metrics_for_error.record_cpal_stream_error()
                }
            },
            None,
        )
        .map_err(|error| format!("failed to build Orange benchmark stream: {error}"))
}

fn fill_callback_body<T, I>(data: &mut [T], source: &mut I) -> CallbackBodyStats
where
    T: cpal::Sample + cpal::FromSample<f32> + PartialEq,
    I: Iterator<Item = f32>,
{
    let mut stats = CallbackBodyStats::default();
    for sample in data.iter_mut() {
        let value = source.next().unwrap_or(0.0);
        if value != 0.0 {
            stats.pre_mute_nonzero += 1;
        }
        stats.pre_mute_peak = stats.pre_mute_peak.max(value.abs());
        *sample = T::from_sample(value);
    }
    let zero = post_dsp_zero();
    for sample in data.iter_mut() {
        *sample = zero;
    }
    stats.post_mute_nonzero = data.iter().filter(|sample| **sample != zero).count() as u64;
    stats
}

fn fill_persistent_callback_body<T>(
    data: &mut [T],
    callback_source: &mut CallbackSource,
    callback_health: &AudioStreamHealth,
    metrics: &CallbackMetrics,
    worker_health: &AtomicU8,
) -> CallbackBodyStats
where
    T: cpal::Sample + cpal::FromSample<f32> + PartialEq,
{
    let Some(source) = callback_source.source_mut() else {
        worker_health.store(
            SourceWorkerHealth::CompletionFailed as u8,
            Ordering::Release,
        );
        mark_persistent_worker_terminal(data, callback_health, metrics);
        return CallbackBodyStats::default();
    };
    worker_health.store(source.source_worker_health() as u8, Ordering::Release);
    if source_worker_health_is_terminal(source.source_worker_health()) {
        mark_persistent_worker_terminal(data, callback_health, metrics);
        return CallbackBodyStats::default();
    }
    let stats = fill_callback_body(data, source);
    let health = source.source_worker_health();
    worker_health.store(health as u8, Ordering::Release);
    if source_worker_health_is_terminal(health) {
        mark_persistent_worker_terminal(data, callback_health, metrics);
    }
    stats
}

fn mark_persistent_worker_terminal<T>(
    data: &mut [T],
    callback_health: &AudioStreamHealth,
    metrics: &CallbackMetrics,
) where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    callback_health.mark_worker_terminal();
    metrics.mark_worker_terminal();
    let zero = post_dsp_zero();
    for sample in data {
        *sample = zero;
    }
}

fn source_worker_health_is_terminal(health: SourceWorkerHealth) -> bool {
    matches!(
        health,
        SourceWorkerHealth::DeadlineMiss
            | SourceWorkerHealth::DispatchFailed
            | SourceWorkerHealth::CompletionFailed
            | SourceWorkerHealth::WorkerExited
            | SourceWorkerHealth::InvalidBlock
    )
}

fn source_worker_health_from_u8(value: u8) -> SourceWorkerHealth {
    match value {
        0 => SourceWorkerHealth::Disabled,
        1 => SourceWorkerHealth::Healthy,
        2 => SourceWorkerHealth::DeadlineMiss,
        3 => SourceWorkerHealth::DispatchFailed,
        4 => SourceWorkerHealth::CompletionFailed,
        5 => SourceWorkerHealth::WorkerExited,
        6 => SourceWorkerHealth::InvalidBlock,
        _ => SourceWorkerHealth::CompletionFailed,
    }
}

fn post_dsp_zero<T>() -> T
where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    T::from_sample(0.0)
}

#[cfg(test)]
mod tests {
    use super::{
        build_persistent_source, expected_worker_thread_names, fill_callback_body, post_dsp_zero,
        source_worker_health_name, stream_geometry, EXECUTOR_MODE,
    };
    use realtime_engine::synth::SourceWorkerHealth;
    use rodio_engine_source::{event_queue, SOURCE_REAPER_THREAD_NAME};

    #[test]
    fn stream_geometry_keeps_output_buffer_and_internal_block_distinct() {
        for (output_frames, internal_frames) in [
            (128, 32),
            (256, 64),
            (256, 128),
            (256, 256),
            (512, 128),
            (1024, 256),
        ] {
            let geometry = stream_geometry(output_frames, internal_frames).unwrap();
            assert_eq!(geometry.output_frames, output_frames);
            assert_eq!(geometry.internal_frames, internal_frames);
        }
        assert!(stream_geometry(512, 256).is_err());
        assert!(stream_geometry(128, 64).is_err());
        assert!(stream_geometry(64, 32).is_err());
    }

    #[test]
    fn benchmark_source_is_persistent_and_reports_worker_health() {
        let (_engine_tx, engine_rx) = event_queue();
        let (source, shutdown_owner) = build_persistent_source(engine_rx, 44_100, 128).unwrap();

        assert_eq!(EXECUTOR_MODE, "persistent_two_workers");
        assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
        assert_eq!(source.block_frames(), 128);
        assert_eq!(
            source_worker_health_name(source.source_worker_health()),
            "healthy"
        );
        assert_eq!(
            expected_worker_thread_names(),
            ["oct-dsp-src-0", "oct-dsp-src-1"]
        );
        assert_eq!(SOURCE_REAPER_THREAD_NAME, "oct-src-reaper");
        assert!(SOURCE_REAPER_THREAD_NAME.len() <= 15);

        drop(source);
        let shutdown = shutdown_owner.shutdown();
        assert_eq!(shutdown.joined_workers, 2);
        assert_eq!(shutdown.retirement_error, None);
    }

    #[test]
    fn benchmark_source_uses_requested_frames() {
        for block_frames in [64, 128, 256, 512, 1024, 2048] {
            let (_engine_tx, engine_rx) = event_queue();
            let (source, shutdown_owner) =
                build_persistent_source(engine_rx, 44_100, block_frames).unwrap();
            assert_eq!(source.block_frames(), block_frames);
            drop(source);
            assert_eq!(shutdown_owner.shutdown().joined_workers, 2);
        }
    }

    #[test]
    fn post_dsp_zero_supports_all_orange_sample_formats() {
        assert_eq!(post_dsp_zero::<f32>(), 0.0);
        assert_eq!(post_dsp_zero::<i16>(), 0);
        assert_eq!(post_dsp_zero::<u16>(), 32_768);
    }

    #[test]
    fn callback_body_consumes_and_mutes_f32_output() {
        let mut data = [1.0_f32; 3];
        let mut source = [0.25, -0.5, 0.0].into_iter();
        let stats = fill_callback_body(&mut data, &mut source);
        assert_eq!(data, [0.0; 3]);
        assert_eq!(source.next(), None);
        assert_eq!(stats.pre_mute_nonzero, 2);
        assert_eq!(stats.post_mute_nonzero, 0);
    }

    #[test]
    fn callback_body_converts_and_mutes_i16_output() {
        let mut data = [1_i16; 3];
        let mut source = [0.25, -0.5, 0.0].into_iter();
        let stats = fill_callback_body(&mut data, &mut source);
        assert_eq!(data, [0; 3]);
        assert_eq!(source.next(), None);
        assert_eq!(stats.pre_mute_nonzero, 2);
    }

    #[test]
    fn callback_body_converts_and_mutes_u16_output() {
        let mut data = [1_u16; 3];
        let mut source = [0.25, -0.5, 0.0].into_iter();
        let stats = fill_callback_body(&mut data, &mut source);
        assert_eq!(data, [32_768; 3]);
        assert_eq!(source.next(), None);
        assert_eq!(stats.pre_mute_nonzero, 2);
    }
}
