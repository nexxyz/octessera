use playback_runtime::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
};
use rodio::Source;
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::Sender;

/// Pulls the desktop output one engine block at a time inside `catch_unwind`. A panic leaves the
/// output silent from then on, never touching the source again, and reports the audio thread as
/// failed once.
pub(crate) struct ContainedAudioSource<S> {
    source: S,
    block: Vec<f32>,
    idx: usize,
    panicked: bool,
    failure_tx: Option<Sender<RuntimeAdapterError>>,
}

impl<S: Source<Item = f32>> ContainedAudioSource<S> {
    pub(crate) fn new(
        source: S,
        block_samples: usize,
        failure_tx: Sender<RuntimeAdapterError>,
    ) -> Self {
        let block_samples = block_samples.max(1);
        Self {
            source,
            block: vec![0.0; block_samples],
            idx: block_samples,
            panicked: false,
            failure_tx: Some(failure_tx),
        }
    }

    fn fill_block(&mut self) {
        self.idx = 0;
        if !self.panicked {
            let (source, block) = (&mut self.source, &mut self.block);
            let filled = catch_unwind(AssertUnwindSafe(|| {
                for sample in block.iter_mut() {
                    *sample = source.next().unwrap_or(0.0);
                }
            }));
            let Err(panic) = filled else {
                return;
            };
            self.panicked = true;
            self.report_panic(panic.as_ref());
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
        self.source.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

#[cfg(test)]
mod tests;
