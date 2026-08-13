use super::metrics::{CallbackMetrics, CallbackPrefix};
use super::phase::{same_measuring_generation, MeasurementControl, MeasurementPhase, PhaseCapture};
use super::probe::ProfileProbe;
use crate::audio_priority::CallbackSchedulingHandle;
use crate::orange_audio::select_orange_stream_config;
use cpal::traits::DeviceTrait;
use cpal::{BufferSize, SampleFormat, Stream, StreamConfig};
use rodio_engine_source::{EngineEventReceiver, EngineSource};
use std::sync::Arc;
use std::time::Instant;

pub struct BenchmarkStream {
    pub stream: Stream,
    pub scheduler: CallbackSchedulingHandle,
    pub sample_format: String,
    pub channels: u16,
    pub sample_rate: u32,
    pub workers_effective: bool,
    pub engine_block_frames: usize,
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

pub fn build(
    engine_rx: EngineEventReceiver,
    output_frames: u32,
    internal_frames: usize,
    workers: usize,
    metrics: Arc<CallbackMetrics>,
    profile_probe: Arc<ProfileProbe>,
    phase_control: Arc<MeasurementControl>,
) -> Result<BenchmarkStream, String> {
    let geometry = stream_geometry(output_frames, internal_frames)?;
    let device = crate::orange_audio::select_orange_output_device()?;
    let (sample_format, mut config) = select_orange_stream_config(&device)?;
    let sample_format_name = format!("{sample_format:?}");
    debug_assert_eq!(geometry.output_frames, output_frames);
    config.buffer_size = BufferSize::Fixed(output_frames);
    let source = EngineSource::with_block_frames_and_workers(
        engine_rx,
        config.sample_rate.0,
        geometry.internal_frames,
        workers,
    );
    let engine_block_frames = source.block_frames();
    let workers_effective = source.synth_slot_parallelism_enabled();
    let scheduler = CallbackSchedulingHandle::new(crate::audio_priority::configured_priority());
    let callback_scheduler = scheduler.clone();
    let stream = match sample_format {
        SampleFormat::F32 => build_typed::<f32>(
            &device,
            &config,
            source,
            callback_scheduler,
            metrics,
            profile_probe,
            phase_control,
        )?,
        SampleFormat::I16 => build_typed::<i16>(
            &device,
            &config,
            source,
            callback_scheduler,
            metrics,
            profile_probe,
            phase_control,
        )?,
        SampleFormat::U16 => build_typed::<u16>(
            &device,
            &config,
            source,
            callback_scheduler,
            metrics,
            profile_probe,
            phase_control,
        )?,
        format => {
            return Err(format!(
                "unsupported Orange benchmark sample format: {format:?}"
            ))
        }
    };
    Ok(BenchmarkStream {
        stream,
        scheduler,
        sample_format: sample_format_name,
        channels: config.channels,
        sample_rate: config.sample_rate.0,
        workers_effective,
        engine_block_frames,
    })
}

fn stream_geometry(output_frames: u32, internal_frames: usize) -> Result<StreamGeometry, String> {
    if !matches!(
        (output_frames, internal_frames),
        (256, 64) | (256, 256) | (512, 128) | (1024, 256)
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
    mut source: EngineSource,
    callback_scheduler: CallbackSchedulingHandle,
    metrics: Arc<CallbackMetrics>,
    profile_probe: Arc<ProfileProbe>,
    phase_control: Arc<MeasurementControl>,
) -> Result<Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32> + PartialEq,
{
    let mut previous_timestamp: Option<cpal::StreamInstant> = None;
    let mut previous_phase: Option<PhaseCapture> = None;
    let channels = config.channels;
    let callback_metrics = metrics.clone();
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
                let body = fill_callback_body(data, &mut source);
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
                    profile_probe.publish(source.profile_snapshot());
                }
                callback_metrics.publish_timing(measured, frames, body_started.elapsed());
            },
            move |error| match error {
                cpal::StreamError::DeviceNotAvailable => metrics.record_cpal_device_error(),
                _ => metrics.record_cpal_stream_error(),
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

fn post_dsp_zero<T>() -> T
where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    T::from_sample(0.0)
}

#[cfg(test)]
mod tests {
    use super::{fill_callback_body, post_dsp_zero, stream_geometry};

    #[test]
    fn stream_geometry_keeps_output_buffer_and_internal_block_distinct() {
        for (output_frames, internal_frames) in [(256, 64), (256, 256), (512, 128), (1024, 256)] {
            let geometry = stream_geometry(output_frames, internal_frames).unwrap();
            assert_eq!(geometry.output_frames, output_frames);
            assert_eq!(geometry.internal_frames, internal_frames);
        }
        assert!(stream_geometry(256, 128).is_err());
        assert!(stream_geometry(512, 256).is_err());
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
