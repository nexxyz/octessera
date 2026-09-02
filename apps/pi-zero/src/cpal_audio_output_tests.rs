use super::{fill_callback, AudioStreamHealth, CallbackSource};
use crate::audio_stream_health::AudioStreamStatus;
use rodio_engine_source::event_queue;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static COUNTING_ENABLED: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static DEALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = System.alloc(layout);
        COUNTING_ENABLED.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.with(|allocations| allocations.set(allocations.get() + 1));
            }
        });
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        COUNTING_ENABLED.with(|enabled| {
            if enabled.get() {
                DEALLOCATIONS.with(|deallocations| deallocations.set(deallocations.get() + 1));
            }
        });
        System.dealloc(pointer, layout);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let pointer = System.realloc(pointer, layout, new_size);
        COUNTING_ENABLED.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.with(|allocations| allocations.set(allocations.get() + 1));
                DEALLOCATIONS.with(|deallocations| deallocations.set(deallocations.get() + 1));
            }
        });
        pointer
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn count_allocations_and_deallocations<F, R>(operation: F) -> (R, usize, usize)
where
    F: FnOnce() -> R,
{
    ALLOCATIONS.with(|allocations| allocations.set(0));
    DEALLOCATIONS.with(|deallocations| deallocations.set(0));
    COUNTING_ENABLED.with(|enabled| enabled.set(true));
    let result = operation();
    COUNTING_ENABLED.with(|enabled| enabled.set(false));
    let allocations = ALLOCATIONS.with(Cell::get);
    let deallocations = DEALLOCATIONS.with(Cell::get);
    (result, allocations, deallocations)
}

#[test]
fn missing_callback_source_silences_output_and_marks_health_terminal() {
    let (_engine_tx, engine_rx) = event_queue();
    let (mut callback_source, _retirement_waiter) =
        CallbackSource::new(super::EngineSource::new(engine_rx, 48_000), false);
    callback_source.source = None;
    let health = AudioStreamHealth::new("test".into());
    let mut output = vec![1.0_f32; 8];
    let mut worker_health_reported = false;

    let (_, allocations, deallocations) = count_allocations_and_deallocations(|| {
        health.with_external_state_lock_for_test(|| {
            fill_callback(
                &mut output,
                &mut callback_source,
                None,
                &health,
                true,
                &mut worker_health_reported,
            );
        });
    });

    assert!(output.iter().all(|sample| sample.to_bits() == 0));
    assert!(health.worker_terminal());
    assert_eq!(health.external_status(), AudioStreamStatus::Healthy);
    assert!(worker_health_reported);
    assert_eq!((allocations, deallocations), (0, 0));
}
