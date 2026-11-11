use super::ContainedAudioSource;
use playback_runtime::{RuntimeErrorCode, RuntimeOperation};
use rodio::Source;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};

struct CountingSource {
    pulled: usize,
    panic_at: Option<usize>,
}

impl Iterator for CountingSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.pulled += 1;
        if self.panic_at == Some(self.pulled) {
            panic!("engine block fault");
        }
        Some(0.5)
    }
}

impl Source for CountingSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        48_000
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

fn shared(panic_at: Option<usize>) -> Arc<Mutex<CountingSource>> {
    Arc::new(Mutex::new(CountingSource {
        pulled: 0,
        panic_at,
    }))
}

fn pulled(source: &Arc<Mutex<CountingSource>>) -> usize {
    source.lock().map_or_else(
        |poisoned| poisoned.into_inner().pulled,
        |source| source.pulled,
    )
}

#[test]
fn a_healthy_source_passes_through_one_block_at_a_time() {
    let (failure_tx, failure_rx) = mpsc::channel();
    let source = shared(None);
    let pulls = Arc::new(AtomicU64::new(0));
    let mut contained = ContainedAudioSource::new(source.clone(), 8, pulls.clone(), failure_tx);

    assert_eq!(contained.next(), Some(0.5));
    assert_eq!(pulled(&source), 8);
    let rest: Vec<f32> = (0..7).map(|_| contained.next().unwrap()).collect();
    assert!(rest.iter().all(|sample| *sample == 0.5));
    assert_eq!(pulled(&source), 8);
    assert_eq!(pulls.load(Ordering::Relaxed), 1);
    assert_eq!(contained.next(), Some(0.5));
    assert_eq!(pulled(&source), 16);
    assert_eq!(pulls.load(Ordering::Relaxed), 2);
    assert_eq!((contained.channels(), contained.sample_rate()), (2, 48_000));
    assert!(failure_rx.try_recv().is_err());
}

#[test]
fn a_reopened_output_keeps_pulling_the_same_engine() {
    let (failure_tx, _failure_rx) = mpsc::channel();
    let source = shared(None);
    let pulls = Arc::new(AtomicU64::new(0));
    let mut lost = ContainedAudioSource::new(source.clone(), 8, pulls.clone(), failure_tx.clone());
    lost.next();
    drop(lost);

    let mut reopened = ContainedAudioSource::new(source.clone(), 8, pulls.clone(), failure_tx);
    reopened.next();

    assert_eq!(pulled(&source), 16);
    assert_eq!(pulls.load(Ordering::Relaxed), 2);
}

#[test]
fn a_panicking_source_goes_silent_for_good_and_reports_audio_failure_once() {
    let (failure_tx, failure_rx) = mpsc::channel();
    let source = shared(Some(11));
    let mut contained =
        ContainedAudioSource::new(source.clone(), 8, Arc::new(AtomicU64::new(0)), failure_tx);

    let first: Vec<f32> = (0..8).map(|_| contained.next().unwrap()).collect();
    assert!(first.iter().all(|sample| *sample == 0.5));
    let after: Vec<f32> = (0..24).map(|_| contained.next().unwrap()).collect();

    assert!(after.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(pulled(&source), 11);
    let failure = failure_rx.try_recv().unwrap();
    assert_eq!(failure.facts.code, RuntimeErrorCode::AudioThreadFailed);
    assert_eq!(failure.facts.operation, RuntimeOperation::AudioThread);
    assert!(failure
        .facts
        .message
        .as_deref()
        .is_some_and(|detail| detail.contains("engine block fault")));
    assert!(failure_rx.try_recv().is_err());
}
