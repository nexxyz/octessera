use super::super::metrics::{CallbackMetrics, CallbackPrefix};
use super::super::phase::{
    same_measuring_generation, MeasurementControl, MeasurementPhase, PhaseCapture,
};
use super::super::probe::ProfileProbe;
use crate::audio::{AudioStreamHealth, CallbackSource};
use crate::audio_priority::CallbackSchedulingHandle;
use cpal::traits::DeviceTrait;
use cpal::{Stream, StreamConfig};
use realtime_engine::synth::{SourceWorkerHealth, SourceWorkerTimingProbe};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CallbackBodyStats {
    pub(crate) pre_mute_nonzero: u64,
    pub(crate) pre_mute_peak: f32,
    pub(crate) post_mute_nonzero: u64,
}

pub(super) struct CallbackContext {
    pub(super) metrics: Arc<CallbackMetrics>,
    pub(super) profile_probe: Arc<ProfileProbe>,
    pub(super) phase_control: Arc<MeasurementControl>,
    pub(super) health: AudioStreamHealth,
    pub(super) worker_health: Arc<AtomicU8>,
    pub(super) timing_probe: Option<Arc<SourceWorkerTimingProbe>>,
}

pub(super) fn build_typed<T>(
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
        timing_probe,
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
                let scheduling_qualified = callback_scheduler.configure_callback_thread();
                if callback_scheduler.is_strict() && !scheduling_qualified {
                    let zero = post_dsp_zero();
                    for sample in data {
                        *sample = zero;
                    }
                    callback_health.mark_callback_terminal();
                    callback_metrics.mark_terminal();
                    return;
                }
                let phase_capture = phase_control.capture_at_callback_entry();
                if phase_control.boundary_pending(phase_capture.generation) {
                    let counters = callback_source
                        .source_mut()
                        .map(|source| source.persistent_output_counters())
                        .unwrap_or_default();
                    callback_metrics.publish_phase_boundary(phase_capture.generation, counters);
                    phase_control.acknowledge(phase_capture);
                }
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
                let callback_elapsed = body_started.elapsed();
                callback_metrics.publish_timing(measured, frames, callback_elapsed);
                if let Some(timing_probe) = timing_probe.as_ref() {
                    timing_probe.record_callback_total(callback_elapsed);
                }
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

pub(crate) fn fill_callback_body<T, I>(data: &mut [T], source: &mut I) -> CallbackBodyStats
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
        let health = SourceWorkerHealth::CompletionFailed;
        worker_health.store(health as u8, Ordering::Release);
        mark_persistent_worker_terminal(data, callback_health, metrics, health);
        return CallbackBodyStats::default();
    };
    let health = source.source_worker_health();
    worker_health.store(health as u8, Ordering::Release);
    if health.is_terminal() {
        mark_persistent_worker_terminal(data, callback_health, metrics, health);
        return CallbackBodyStats::default();
    }
    let stats = fill_callback_body(data, source);
    let health = source.source_worker_health();
    worker_health.store(health as u8, Ordering::Release);
    if health.is_terminal() {
        mark_persistent_worker_terminal(data, callback_health, metrics, health);
    }
    stats
}

fn mark_persistent_worker_terminal<T>(
    data: &mut [T],
    callback_health: &AudioStreamHealth,
    metrics: &CallbackMetrics,
    health: SourceWorkerHealth,
) where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    callback_health.mark_worker_health(health);
    metrics.mark_worker_terminal();
    let zero = post_dsp_zero();
    for sample in data {
        *sample = zero;
    }
}

pub(crate) fn post_dsp_zero<T>() -> T
where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    T::from_sample(0.0)
}
