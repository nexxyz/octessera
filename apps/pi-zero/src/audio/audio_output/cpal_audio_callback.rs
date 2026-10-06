use super::audio_stream_lifecycle::{AudioStreamRetirementError, AudioStreamRetirementWaiter};
use super::MixTapState;
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
    mix_taps: Option<&MixTapState>,
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
    fill_output(data, source, mix_taps);
    let health = source.source_worker_health();
    if report_worker_health && !*worker_health_reported && health.is_terminal() {
        mark_worker_terminal(data, callback_health, health);
        *worker_health_reported = true;
    }
}

pub(super) fn fill_callback_with_scheduler<T>(
    data: &mut [T],
    callback_source: &mut CallbackSource,
    mix_taps: Option<&MixTapState>,
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
        mix_taps,
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

fn fill_output<T>(data: &mut [T], source: &mut EngineSource, mix_taps: Option<&MixTapState>)
where
    T: Sample + cpal::FromSample<f32>,
{
    let taps_guard = mix_taps.and_then(|taps| taps.try_read().ok());
    let taps = taps_guard.as_ref().map_or([None, None], |taps| {
        [taps.recording.as_ref(), taps.monitor.as_ref()]
    });
    if taps.iter().all(Option::is_none) {
        for sample in data.iter_mut() {
            *sample = T::from_sample(source.next().unwrap_or(0.0));
        }
        return;
    }
    let mut chunks = taps.map(|tap| tap.map(|tap| tap.new_chunk()));
    let (frames, remainder) = data.as_chunks_mut::<2>();
    for frame in frames {
        let left = source.next().unwrap_or(0.0);
        let right = source.next().unwrap_or(0.0);
        for (tap, chunk) in taps.iter().zip(chunks.iter_mut()) {
            if let (Some(tap), Some(chunk)) = (tap, chunk.as_mut()) {
                push_tap_frame(tap, chunk, float_to_i16(left), float_to_i16(right));
            }
        }
        frame[0] = T::from_sample(left);
        frame[1] = T::from_sample(right);
    }
    for sample in remainder {
        let value = source.next().unwrap_or(0.0);
        *sample = T::from_sample(value);
    }
    for (tap, chunk) in taps.iter().zip(chunks) {
        if let (Some(tap), Some(chunk)) = (tap, chunk) {
            if !chunk.is_empty() {
                tap.push_chunk(chunk);
            }
        }
    }
}

fn push_tap_frame(
    tap: &media_recording::RecordingTap,
    chunk: &mut media_recording::RecordingChunk,
    left: i16,
    right: i16,
) {
    if !chunk.push_frame(left, right) {
        let full = std::mem::replace(chunk, media_recording::RecordingChunk::new(0));
        tap.push_chunk(full);
        *chunk = tap.new_chunk();
        let _ = chunk.push_frame(left, right);
    }
}

fn float_to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

#[cfg(test)]
mod mirror_tests;
#[cfg(test)]
mod tests;
