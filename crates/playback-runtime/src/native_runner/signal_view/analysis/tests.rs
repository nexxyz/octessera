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
