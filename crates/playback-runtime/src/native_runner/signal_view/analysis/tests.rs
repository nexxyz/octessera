use super::*;
use std::time::Instant;

#[test]
fn dropping_the_worker_does_not_wait_for_its_lock_and_the_worker_still_stops() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    let handle = WorkerHandle::spawn(tap);
    let shared = Arc::clone(&handle.shared);
    let held = shared.lock();
    drop(handle);
    drop(held);

    let deadline = Instant::now() + STOP_CHECK * 5;
    while Arc::strong_count(&shared) > 1 {
        assert!(Instant::now() < deadline, "the worker did not stop");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_worker_that_could_not_start_shows_the_view_as_unavailable() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    let unstarted = WorkerHandle {
        shared: Arc::new(Shared {
            slot: Mutex::new(Slot::default()),
            wake: Condvar::new(),
            stopped: AtomicBool::new(true),
        }),
    };
    let analyzer = Analyzer::Worker {
        tap,
        worker: Some(Arc::new(unstarted)),
    };
    let reader = super::super::SignalTapReader::with_analyzer(tap, analyzer);
    let view = LiveView {
        source: realtime_engine::synth::SignalSource::Master,
        kind: VisualKind::Scope,
        tuning: super::super::VisualTuning::DEFAULT,
        rows: crate::oled_frame::SIGNAL_BAND_HEIGHT,
    };
    let shown = reader.visual(Some(view), Instant::now());
    assert_eq!(*shown, OledSignalVisual::Unavailable);
    assert_eq!(*reader.visual(None, Instant::now()), OledSignalVisual::Flat);
}
