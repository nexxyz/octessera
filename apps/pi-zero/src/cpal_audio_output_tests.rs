use super::{
    fill_callback, fill_callback_with_scheduler, mark_worker_terminal, AudioStreamHealth,
    CallbackSource,
};
use crate::audio_priority::{
    install_blocked_test_scheduling, install_test_scheduling, CallbackSchedulingHandle, CpuMask,
    InjectedSchedulingOutcomes, SchedulingFailureStage, SchedulingSyscall,
};
use crate::audio_stream_health::AudioStreamStatus;
use realtime_engine::synth::SourceWorkerHealth;
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
    assert_eq!(health.worker_health(), SourceWorkerHealth::CompletionFailed);
    assert_eq!(health.external_status(), AudioStreamStatus::Healthy);
    assert!(worker_health_reported);
    assert_eq!((allocations, deallocations), (0, 0));
}

#[test]
fn callback_keeps_deadline_miss_silent_without_terminal_health_or_allocation() {
    let health = AudioStreamHealth::new("test".into());
    let mut output = vec![1.0_f32; 8];
    let (_, allocations, deallocations) = count_allocations_and_deallocations(|| {
        mark_worker_terminal(&mut output, &health, SourceWorkerHealth::DeadlineMiss);
    });

    assert!(output.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(health.worker_health(), SourceWorkerHealth::Healthy);
    assert_eq!(health.runtime_status(), AudioStreamStatus::Healthy);
    assert_eq!((allocations, deallocations), (0, 0));
}

#[test]
fn callback_reports_each_exact_structural_terminal_reason_without_allocation() {
    for reason in [
        SourceWorkerHealth::DispatchFailed,
        SourceWorkerHealth::CompletionFailed,
        SourceWorkerHealth::WorkerExited,
        SourceWorkerHealth::InvalidBlock,
    ] {
        let health = AudioStreamHealth::new("test".into());
        let mut output = vec![1.0_f32; 8];
        let (_, allocations, deallocations) = count_allocations_and_deallocations(|| {
            mark_worker_terminal(&mut output, &health, reason);
        });

        assert!(output.iter().all(|sample| sample.to_bits() == 0));
        assert_eq!(health.worker_health(), reason);
        assert_eq!(health.runtime_status(), AudioStreamStatus::Terminal);
        assert_eq!((allocations, deallocations), (0, 0));
    }
}

#[test]
fn strict_callback_qualifies_before_consuming_the_first_sample_and_only_once() {
    let guard = install_test_scheduling(InjectedSchedulingOutcomes::success_for_cpu(1));
    let (engine_tx, engine_rx) = event_queue();
    engine_tx
        .send(rodio_engine_source::EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 1_000,
        })
        .unwrap();
    let (mut callback_source, _retirement_waiter) = CallbackSource::new(
        super::EngineSource::with_block_frames(engine_rx, 48_000, 128),
        false,
    );
    let health = AudioStreamHealth::new("Jack".into());
    let scheduler = CallbackSchedulingHandle::new_orange_jack();
    let mut reported = false;
    let mut output = [1.0_f32; 8];

    fill_callback_with_scheduler(
        &mut output,
        &mut callback_source,
        None,
        &health,
        false,
        &mut reported,
        &scheduler,
    );
    assert_eq!(
        callback_source
            .source_mut()
            .unwrap()
            .profile_snapshot()
            .active_synth_voices,
        1
    );
    assert_eq!(
        guard.trace_for_cpu(1),
        vec![
            SchedulingSyscall::SetAffinity,
            SchedulingSyscall::GetAffinity,
            SchedulingSyscall::SetScheduling,
            SchedulingSyscall::GetScheduling,
        ]
    );
    let setup_trace = guard.trace_for_cpu(1);

    fill_callback_with_scheduler(
        &mut output,
        &mut callback_source,
        None,
        &health,
        false,
        &mut reported,
        &scheduler,
    );
    assert!(
        callback_source
            .source_mut()
            .unwrap()
            .profile_snapshot()
            .active_synth_voices
            >= 1
    );
    assert_eq!(guard.trace_for_cpu(1), setup_trace);
    assert!(health.external_status() == crate::audio_stream_health::AudioStreamStatus::Healthy);
}

#[test]
fn strict_callback_failure_silences_without_source_access_or_retry() {
    let mut outcomes = InjectedSchedulingOutcomes::success();
    outcomes.target_cpu = Some(1);
    outcomes.observed_affinity = Some(CpuMask::single(1) | CpuMask::single(4));
    let guard = install_test_scheduling(outcomes);
    let (engine_tx, engine_rx) = event_queue();
    engine_tx
        .send(rodio_engine_source::EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 1_000,
        })
        .unwrap();
    let (mut callback_source, _retirement_waiter) = CallbackSource::new(
        super::EngineSource::with_block_frames(engine_rx, 48_000, 128),
        false,
    );
    let health = AudioStreamHealth::new("Jack".into());
    let scheduler = CallbackSchedulingHandle::new_orange_jack();
    let mut reported = false;
    let mut output = [1.0_f32; 8];

    for _ in 0..2 {
        fill_callback_with_scheduler(
            &mut output,
            &mut callback_source,
            None,
            &health,
            false,
            &mut reported,
            &scheduler,
        );
        assert!(output.iter().all(|sample| sample.to_bits() == 0));
        assert_eq!(
            callback_source
                .source_mut()
                .unwrap()
                .profile_snapshot()
                .active_synth_voices,
            0
        );
    }
    assert_eq!(
        health.external_status(),
        crate::audio_stream_health::AudioStreamStatus::Terminal
    );
    assert_eq!(
        guard.trace_for_cpu(1),
        vec![
            SchedulingSyscall::SetAffinity,
            SchedulingSyscall::GetAffinity,
        ]
    );
    assert!(matches!(
        scheduler.status(),
        crate::audio_priority::CallbackSchedulingStatus::Failed(failure)
            if failure.stage == SchedulingFailureStage::AffinityMismatch
    ));
}

#[test]
fn strict_callback_setup_and_silence_do_not_allocate_or_deallocate() {
    let _guard = install_test_scheduling(InjectedSchedulingOutcomes::success_for_cpu(1));
    let (_engine_tx, engine_rx) = event_queue();
    let (mut callback_source, _retirement_waiter) = CallbackSource::new(
        super::EngineSource::with_block_frames(engine_rx, 48_000, 128),
        false,
    );
    let health = AudioStreamHealth::new("Jack".into());
    let scheduler = CallbackSchedulingHandle::new_orange_jack();
    let mut reported = false;
    let mut output = [1.0_f32; 8];
    let (_, allocations, deallocations) = count_allocations_and_deallocations(|| {
        fill_callback_with_scheduler(
            &mut output,
            &mut callback_source,
            None,
            &health,
            false,
            &mut reported,
            &scheduler,
        );
    });

    assert_eq!((allocations, deallocations), (0, 0));
}

#[test]
fn strict_timeout_records_terminal_failure_without_running_setup() {
    let guard = install_test_scheduling(InjectedSchedulingOutcomes::success_for_cpu(1));
    let scheduler = CallbackSchedulingHandle::new_orange_jack();
    let error = crate::audio_priority::qualify_callback_scheduler(
        "Jack",
        &scheduler,
        std::time::Duration::ZERO,
    )
    .unwrap_err();
    assert!(error.contains("stage=timeout"));
    assert!(guard.trace_for_cpu(1).is_empty());
}

#[test]
fn strict_timeout_wins_during_setup_without_source_advance_or_retry() {
    let (guard, block) = install_blocked_test_scheduling(
        InjectedSchedulingOutcomes::success_for_cpu(1),
        SchedulingSyscall::SetAffinity,
    );
    let (engine_tx, engine_rx) = event_queue();
    engine_tx
        .send(rodio_engine_source::EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 1_000,
        })
        .unwrap();
    let (callback_source, _retirement_waiter) = CallbackSource::new(
        super::EngineSource::with_block_frames(engine_rx, 48_000, 128),
        false,
    );
    let health = AudioStreamHealth::new("Jack".into());
    let scheduler = CallbackSchedulingHandle::new_orange_jack();
    let callback_scheduler = scheduler.clone();
    let callback_health = health.clone();
    let callback_thread = std::thread::spawn(move || {
        let mut callback_source = callback_source;
        let mut reported = false;
        let mut output = [1.0_f32; 8];
        fill_callback_with_scheduler(
            &mut output,
            &mut callback_source,
            None,
            &callback_health,
            false,
            &mut reported,
            &callback_scheduler,
        );
        (output, callback_source)
    });

    block.wait_until_entered();
    let expected_error = "Jack audio callback RT placement not qualified: stage=timeout errno=0 requested_cpu=1 requested_policy=SCHED_FIFO requested_priority=70 observed_mask=0x0 observed_policy=0 observed_priority=0";
    assert_eq!(
        crate::audio_priority::qualify_callback_scheduler(
            "Jack",
            &scheduler,
            std::time::Duration::ZERO,
        ),
        Err(expected_error.into())
    );
    assert!(matches!(
        scheduler.status(),
        crate::audio_priority::CallbackSchedulingStatus::TimedOut
    ));

    block.release();
    let (output, mut callback_source) = callback_thread.join().unwrap();
    assert!(output.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(
        callback_source
            .source_mut()
            .unwrap()
            .profile_snapshot()
            .active_synth_voices,
        0
    );
    let trace = guard.trace_for_cpu(1);
    assert_eq!(
        trace,
        vec![
            SchedulingSyscall::SetAffinity,
            SchedulingSyscall::GetAffinity,
            SchedulingSyscall::SetScheduling,
            SchedulingSyscall::GetScheduling,
        ]
    );

    let mut reported = false;
    let mut second_output = [1.0_f32; 8];
    fill_callback_with_scheduler(
        &mut second_output,
        &mut callback_source,
        None,
        &health,
        false,
        &mut reported,
        &scheduler,
    );
    assert!(second_output.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(guard.trace_for_cpu(1), trace);
    assert_eq!(health.external_status(), AudioStreamStatus::Terminal);
    assert_eq!(
        crate::audio_priority::qualify_callback_scheduler(
            "Jack",
            &scheduler,
            std::time::Duration::from_millis(250),
        ),
        Err(expected_error.into())
    );
}
