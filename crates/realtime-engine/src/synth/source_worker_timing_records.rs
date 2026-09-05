use super::{duration_ns, NO_CPU, NO_SEQUENCE, SOURCE_WORKER_COUNT, TIMING_UNSET};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::Duration;

#[repr(C, align(64))]
pub(super) struct WorkerTimingRecord {
    pub(super) sequence: AtomicU64,
    pub(super) render_ns: AtomicU64,
    pub(super) dispatch_to_finish_ns: AtomicU64,
    pub(super) cpu_start: AtomicU32,
    pub(super) cpu_end: AtomicU32,
    pub(super) finished: AtomicBool,
}

impl WorkerTimingRecord {
    fn new() -> Self {
        Self {
            sequence: AtomicU64::new(NO_SEQUENCE),
            render_ns: AtomicU64::new(0),
            dispatch_to_finish_ns: AtomicU64::new(0),
            cpu_start: AtomicU32::new(NO_CPU),
            cpu_end: AtomicU32::new(NO_CPU),
            finished: AtomicBool::new(false),
        }
    }
}

#[repr(C, align(64))]
pub(super) struct CoordinatorTimingRecord {
    pub(super) sequence: AtomicU64,
    pub(super) sequence_valid: AtomicBool,
    pub(super) dispatch_to_deadline_start_ns: AtomicU64,
    pub(super) dispatch_to_deadline_start_valid: AtomicBool,
    pub(super) deadline_ns: AtomicU64,
    pub(super) dispatch_to_deadline_elapsed_ns: AtomicU64,
    pub(super) dispatch_to_deadline_elapsed_valid: AtomicBool,
    pub(super) in_flight_mask: AtomicU8,
    pub(super) completed_mask: AtomicU8,
    pub(super) first_parity: AtomicU8,
    pub(super) first_parity_valid: AtomicBool,
    pub(super) dispatch_to_first_ns: AtomicU64,
    pub(super) dispatch_to_first_valid: AtomicBool,
    pub(super) dispatch_to_both_ns: AtomicU64,
    pub(super) dispatch_to_both_valid: AtomicBool,
    pub(super) reduction_ns: AtomicU64,
    pub(super) reduction_valid: AtomicBool,
    pub(super) coordinator_remainder_ns: AtomicU64,
    pub(super) coordinator_remainder_valid: AtomicBool,
    pub(super) engine_block_total_ns: AtomicU64,
    pub(super) engine_block_total_state: AtomicU8,
    pub(super) callback_total_ns: AtomicU64,
    pub(super) callback_total_state: AtomicU8,
    pub(super) fully_completed: AtomicBool,
    pub(super) failed: AtomicBool,
    pub(super) frozen: AtomicBool,
}

impl CoordinatorTimingRecord {
    fn new() -> Self {
        Self {
            sequence: AtomicU64::new(NO_SEQUENCE),
            sequence_valid: AtomicBool::new(false),
            dispatch_to_deadline_start_ns: AtomicU64::new(0),
            dispatch_to_deadline_start_valid: AtomicBool::new(false),
            deadline_ns: AtomicU64::new(0),
            dispatch_to_deadline_elapsed_ns: AtomicU64::new(0),
            dispatch_to_deadline_elapsed_valid: AtomicBool::new(false),
            in_flight_mask: AtomicU8::new(0),
            completed_mask: AtomicU8::new(0),
            first_parity: AtomicU8::new(0),
            first_parity_valid: AtomicBool::new(false),
            dispatch_to_first_ns: AtomicU64::new(0),
            dispatch_to_first_valid: AtomicBool::new(false),
            dispatch_to_both_ns: AtomicU64::new(0),
            dispatch_to_both_valid: AtomicBool::new(false),
            reduction_ns: AtomicU64::new(0),
            reduction_valid: AtomicBool::new(false),
            coordinator_remainder_ns: AtomicU64::new(0),
            coordinator_remainder_valid: AtomicBool::new(false),
            engine_block_total_ns: AtomicU64::new(0),
            engine_block_total_state: AtomicU8::new(TIMING_UNSET),
            callback_total_ns: AtomicU64::new(0),
            callback_total_state: AtomicU8::new(TIMING_UNSET),
            fully_completed: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            frozen: AtomicBool::new(false),
        }
    }

    pub(super) fn reset(
        &self,
        sequence: u64,
        deadline: Duration,
        workers: &[WorkerTimingRecord; SOURCE_WORKER_COUNT],
    ) {
        self.sequence_valid.store(false, Ordering::Release);
        for worker in workers {
            worker.sequence.store(NO_SEQUENCE, Ordering::Release);
            worker.finished.store(false, Ordering::Release);
            worker.render_ns.store(0, Ordering::Relaxed);
            worker.dispatch_to_finish_ns.store(0, Ordering::Relaxed);
            worker.cpu_start.store(NO_CPU, Ordering::Relaxed);
            worker.cpu_end.store(NO_CPU, Ordering::Relaxed);
        }
        self.dispatch_to_deadline_start_valid
            .store(false, Ordering::Relaxed);
        self.deadline_ns
            .store(duration_ns(deadline), Ordering::Relaxed);
        self.dispatch_to_deadline_elapsed_valid
            .store(false, Ordering::Relaxed);
        self.in_flight_mask.store(0, Ordering::Relaxed);
        self.completed_mask.store(0, Ordering::Relaxed);
        self.first_parity_valid.store(false, Ordering::Relaxed);
        self.dispatch_to_first_valid.store(false, Ordering::Relaxed);
        self.dispatch_to_both_valid.store(false, Ordering::Relaxed);
        self.reduction_valid.store(false, Ordering::Relaxed);
        self.coordinator_remainder_valid
            .store(false, Ordering::Relaxed);
        self.engine_block_total_state
            .store(TIMING_UNSET, Ordering::Relaxed);
        self.callback_total_state
            .store(TIMING_UNSET, Ordering::Relaxed);
        self.fully_completed.store(false, Ordering::Relaxed);
        self.failed.store(false, Ordering::Relaxed);
        self.frozen.store(false, Ordering::Relaxed);
        self.sequence.store(sequence, Ordering::Relaxed);
        self.sequence_valid.store(true, Ordering::Release);
    }
}

#[repr(C, align(64))]
pub(super) struct SequenceTimingRecord {
    pub(super) workers: [WorkerTimingRecord; SOURCE_WORKER_COUNT],
    pub(super) coordinator: CoordinatorTimingRecord,
}

impl SequenceTimingRecord {
    pub(super) fn new() -> Self {
        Self {
            workers: std::array::from_fn(|_| WorkerTimingRecord::new()),
            coordinator: CoordinatorTimingRecord::new(),
        }
    }
}
