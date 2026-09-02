use super::audio_stream_lifecycle::{AudioStreamRetirementError, AudioStreamRetirementWaiter};
use super::cpal_audio_output::source_worker_health_is_terminal;
use super::RecordingTapState;
use crate::audio_stream_health::AudioStreamHealth;
use cpal::Sample;
use rodio_engine_source::EngineSource;
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
        silence_output(data);
        callback_health.mark_worker_terminal();
        *worker_health_reported = true;
        return;
    };
    if report_worker_health && source_worker_health_is_terminal(source.source_worker_health()) {
        silence_output(data);
        callback_health.mark_worker_terminal();
        *worker_health_reported = true;
        return;
    }
    fill_output(data, source, recording_tap);
    if report_worker_health
        && !*worker_health_reported
        && source_worker_health_is_terminal(source.source_worker_health())
    {
        silence_output(data);
        callback_health.mark_worker_terminal();
        *worker_health_reported = true;
    }
}

fn silence_output<T>(data: &mut [T])
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
#[path = "cpal_audio_output_tests.rs"]
mod tests;
