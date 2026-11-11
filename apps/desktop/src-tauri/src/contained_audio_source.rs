use playback_runtime::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
};
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio::Source;
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// Pulls the desktop output one engine block at a time inside `catch_unwind`. A panic leaves the
/// output silent from then on, never touching the source again, and reports the audio thread as
/// failed once. The source is shared, so a reopened output keeps pulling the same engine;
/// `pulls` counts the blocks, which tells whether the output is still running.
pub(crate) struct ContainedAudioSource<S> {
    source: Arc<Mutex<S>>,
    channels: u16,
    sample_rate: u32,
    block: Vec<f32>,
    idx: usize,
    panicked: bool,
    pulls: Arc<AtomicU64>,
    failure_tx: Option<Sender<RuntimeAdapterError>>,
}

impl<S: Source<Item = f32>> ContainedAudioSource<S> {
    pub(crate) fn new(
        source: Arc<Mutex<S>>,
        block_samples: usize,
        pulls: Arc<AtomicU64>,
        failure_tx: Sender<RuntimeAdapterError>,
    ) -> Self {
        let block_samples = block_samples.max(1);
        let (channels, sample_rate) = source
            .lock()
            .map_or((2, DEFAULT_AUDIO_SAMPLE_RATE), |source| {
                (source.channels(), source.sample_rate())
            });
        Self {
            source,
            channels,
            sample_rate,
            block: vec![0.0; block_samples],
            idx: block_samples,
            panicked: false,
            pulls,
            failure_tx: Some(failure_tx),
        }
    }

    fn fill_block(&mut self) {
        self.idx = 0;
        self.pulls.fetch_add(1, Ordering::Relaxed);
        if !self.panicked {
            let filled = match self.source.try_lock() {
                Ok(mut source) => {
                    let block = &mut self.block;
                    Some(catch_unwind(AssertUnwindSafe(|| {
                        for sample in block.iter_mut() {
                            *sample = source.next().unwrap_or(0.0);
                        }
                    })))
                }
                Err(_) => None,
            };
            match filled {
                Some(Ok(())) => return,
                Some(Err(panic)) => {
                    self.panicked = true;
                    self.report_panic(panic.as_ref());
                }
                None => {}
            }
        }
        self.block.fill(0.0);
    }

    fn report_panic(&mut self, panic: &(dyn Any + Send)) {
        let Some(failure_tx) = self.failure_tx.take() else {
            return;
        };
        let _ = failure_tx.send(RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::AudioThreadFailed,
            RuntimeOperation::AudioThread,
            Some(format!("audio source panic: {}", panic_message(panic))),
        )));
    }
}

pub(crate) fn panic_message(panic: &(dyn Any + Send)) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic")
}

impl<S: Source<Item = f32>> Iterator for ContainedAudioSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.block.len() {
            self.fill_block();
        }
        let sample = self.block[self.idx];
        self.idx += 1;
        Some(sample)
    }
}

impl<S: Source<Item = f32>> Source for ContainedAudioSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

#[cfg(test)]
mod tests;
