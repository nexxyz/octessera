use super::audio_stream_lifecycle::{AudioStreamRetirementError, AudioStreamRetirementWaiter};
use super::RecordingTapState;
use crate::audio_priority::CallbackSchedulingHandle;
use crate::audio_stream_health::AudioStreamHealth;
use cpal::Sample;
use realtime_engine::synth::SourceWorkerHealth;
use rodio_engine_source::{EngineSource, PcmMirrorConsumer};
use std::sync::mpsc;

pub(crate) struct CallbackSource {
    source: Option<EngineSource>,
    retired_tx: Option<mpsc::SyncSender<()>>,
}

impl CallbackSource {
    pub(crate) fn new(
        source: EngineSource,
        wait_for_retirement: bool,
    ) -> (Self, Option<AudioStreamRetirementWaiter>) {
        let (retired_tx, waiter) = if wait_for_retirement {
            let (retired_tx, retired_rx) = mpsc::sync_channel(1);
            let waiter: AudioStreamRetirementWaiter = Box::new(move || {
                retired_rx
                    .recv()
                    .map_err(|_| AudioStreamRetirementError::CallbackSourceUnavailable)
            });
            (Some(retired_tx), Some(waiter))
        } else {
            (None, None)
        };
        (
            Self {
                source: Some(source),
                retired_tx,
            },
            waiter,
        )
    }

    pub(crate) fn source_mut(&mut self) -> Option<&mut EngineSource> {
        self.source.as_mut()
    }
}

impl Drop for CallbackSource {
    fn drop(&mut self) {
        drop(self.source.take());
        if let Some(retired_tx) = self.retired_tx.take() {
            let _ = retired_tx.try_send(());
        }
    }
}

pub(crate) struct MirrorCallbackSource {
    consumer: PcmMirrorConsumer,
}

impl MirrorCallbackSource {
    pub(crate) fn new(consumer: PcmMirrorConsumer) -> Self {
        Self { consumer }
    }

    fn consumer_mut(&mut self) -> &mut PcmMirrorConsumer {
        &mut self.consumer
    }
}

pub(super) fn fill_callback<T>(
    data: &mut [T],
    callback_source: &mut CallbackSource,
    recording_tap: Option<&RecordingTapState>,
    callback_health: &AudioStreamHealth,
    report_worker_health: bool,
    worker_health_reported: &mut bool,
) where
    T: Sample + cpal::FromSample<f32>,
{
    let Some(source) = callback_source.source_mut() else {
        mark_worker_terminal(data, callback_health, SourceWorkerHealth::CompletionFailed);
        *worker_health_reported = true;
        return;
    };
    let health = source.source_worker_health();
    if report_worker_health && health.is_terminal() {
        mark_worker_terminal(data, callback_health, health);
        *worker_health_reported = true;
        return;
    }
    fill_output(data, source, recording_tap);
    let health = source.source_worker_health();
    if report_worker_health && !*worker_health_reported && health.is_terminal() {
        mark_worker_terminal(data, callback_health, health);
        *worker_health_reported = true;
    }
}

pub(super) fn fill_callback_with_scheduler<T>(
    data: &mut [T],
    callback_source: &mut CallbackSource,
    recording_tap: Option<&RecordingTapState>,
    callback_health: &AudioStreamHealth,
    report_worker_health: bool,
    worker_health_reported: &mut bool,
    scheduler: &CallbackSchedulingHandle,
) where
    T: Sample + cpal::FromSample<f32>,
{
    if !scheduler.configure_callback_thread() {
        silence_output(data);
        callback_health.mark_callback_terminal();
        return;
    }
    fill_callback(
        data,
        callback_source,
        recording_tap,
        callback_health,
        report_worker_health,
        worker_health_reported,
    );
}

pub(super) fn fill_mirror_callback_with_scheduler<T>(
    data: &mut [T],
    callback_source: &mut MirrorCallbackSource,
    scheduler: &CallbackSchedulingHandle,
) where
    T: Sample + cpal::FromSample<f32>,
{
    if !scheduler.configure_callback_thread() {
        silence_output(data);
        return;
    }
    if !data.len().is_multiple_of(2) {
        silence_output(data);
        return;
    }
    if !callback_source.consumer_mut().begin_callback() {
        silence_output(data);
        return;
    }
    let mut index = 0;
    while index < data.len() {
        let Some(value) = callback_source.consumer_mut().next_sample() else {
            silence_output(&mut data[index..]);
            return;
        };
        data[index] = T::from_sample(value);
        index += 1;
    }
}

pub(super) fn mark_worker_terminal<T>(
    data: &mut [T],
    callback_health: &AudioStreamHealth,
    health: SourceWorkerHealth,
) where
    T: Sample + cpal::FromSample<f32>,
{
    silence_output(data);
    callback_health.mark_worker_health(health);
}

pub(super) fn silence_output<T>(data: &mut [T])
where
    T: Sample + cpal::FromSample<f32>,
{
    for sample in data {
        *sample = T::from_sample(0.0);
    }
}

fn fill_output<T>(
    data: &mut [T],
    source: &mut EngineSource,
    recording_tap: Option<&RecordingTapState>,
) where
    T: Sample + cpal::FromSample<f32>,
{
    let recording_tap_guard = recording_tap.and_then(|tap| tap.try_read().ok());
    let recorded = recording_tap_guard
        .as_ref()
        .and_then(|tap| (**tap).as_ref());
    let mut recording_chunk = recorded
        .as_ref()
        .map(|_| crate::recording::RecordingChunk::new());
    for sample in data {
        let value = source.next().unwrap_or(0.0);
        if let (Some(tap), Some(chunk)) = (recorded.as_ref(), recording_chunk.as_mut()) {
            if !chunk.push(float_to_i16(value)) {
                tap.push_chunk(chunk.take());
                let _ = chunk.push(float_to_i16(value));
            }
        }
        *sample = T::from_sample(value);
    }
    if let (Some(tap), Some(chunk)) = (recorded.as_ref(), recording_chunk) {
        if !chunk.is_empty() {
            tap.push_chunk(chunk);
        }
    }
}

fn float_to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

#[cfg(test)]
#[path = "cpal_audio_mirror_tests.rs"]
mod mirror_tests;
#[cfg(test)]
#[path = "cpal_audio_output_tests.rs"]
mod tests;
