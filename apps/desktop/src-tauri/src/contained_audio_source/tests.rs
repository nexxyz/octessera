use super::ContainedAudioSource;
use playback_runtime::{RuntimeErrorCode, RuntimeOperation};
use rodio::Source;
use std::sync::mpsc;

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

#[test]
fn a_healthy_source_passes_through_one_block_at_a_time() {
    let (failure_tx, failure_rx) = mpsc::channel();
    let source = CountingSource {
        pulled: 0,
        panic_at: None,
    };
    let mut contained = ContainedAudioSource::new(source, 8, failure_tx);

    assert_eq!(contained.next(), Some(0.5));
    assert_eq!(contained.source.pulled, 8);
    let rest: Vec<f32> = (0..7).map(|_| contained.next().unwrap()).collect();
    assert!(rest.iter().all(|sample| *sample == 0.5));
    assert_eq!(contained.source.pulled, 8);
    assert_eq!(contained.next(), Some(0.5));
    assert_eq!(contained.source.pulled, 16);
    assert_eq!((contained.channels(), contained.sample_rate()), (2, 48_000));
    assert!(failure_rx.try_recv().is_err());
}

#[test]
fn a_panicking_source_goes_silent_for_good_and_reports_audio_failure_once() {
    let (failure_tx, failure_rx) = mpsc::channel();
    let source = CountingSource {
        pulled: 0,
        panic_at: Some(11),
    };
    let mut contained = ContainedAudioSource::new(source, 8, failure_tx);

    let first: Vec<f32> = (0..8).map(|_| contained.next().unwrap()).collect();
    assert!(first.iter().all(|sample| *sample == 0.5));
    let after: Vec<f32> = (0..24).map(|_| contained.next().unwrap()).collect();

    assert!(after.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(contained.source.pulled, 11);
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
