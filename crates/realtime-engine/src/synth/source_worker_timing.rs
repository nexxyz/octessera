use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::{Duration, Instant};

const SOURCE_WORKER_COUNT: usize = 2;
const NO_CPU: u32 = u32::MAX;
const TIMING_UNSET: u8 = 0;
const TIMING_WRITING: u8 = 1;
const TIMING_SET: u8 = 2;

pub type SourceWorkerCpuSampler = fn() -> u32;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceWorkerTimingStart {
    started_at: Instant,
    cpu_start: u32,
}

#[repr(C, align(64))]
struct WorkerTimingRecord {
    sequence: AtomicU64,
    render_ns: AtomicU64,
    dispatch_to_finish_ns: AtomicU64,
    cpu_start: AtomicU32,
    cpu_end: AtomicU32,
    finished: AtomicBool,
}

impl WorkerTimingRecord {
    fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            render_ns: AtomicU64::new(0),
            dispatch_to_finish_ns: AtomicU64::new(0),
            cpu_start: AtomicU32::new(NO_CPU),
            cpu_end: AtomicU32::new(NO_CPU),
            finished: AtomicBool::new(false),
        }
    }
}

#[repr(C, align(64))]
struct CoordinatorTimingRecord {
    sequence: AtomicU64,
    sequence_valid: AtomicBool,
    dispatch_to_deadline_start_ns: AtomicU64,
    dispatch_to_deadline_start_valid: AtomicBool,
    deadline_ns: AtomicU64,
    dispatch_to_deadline_elapsed_ns: AtomicU64,
    dispatch_to_deadline_elapsed_valid: AtomicBool,
    in_flight_mask: AtomicU8,
    completed_mask: AtomicU8,
    first_parity: AtomicU8,
    first_parity_valid: AtomicBool,
    dispatch_to_first_ns: AtomicU64,
    dispatch_to_first_valid: AtomicBool,
    dispatch_to_both_ns: AtomicU64,
    dispatch_to_both_valid: AtomicBool,
    reduction_ns: AtomicU64,
    reduction_valid: AtomicBool,
    coordinator_remainder_ns: AtomicU64,
    coordinator_remainder_valid: AtomicBool,
    engine_block_total_ns: AtomicU64,
    engine_block_total_state: AtomicU8,
    callback_total_ns: AtomicU64,
    callback_total_state: AtomicU8,
    failed: AtomicBool,
    frozen: AtomicBool,
}

impl CoordinatorTimingRecord {
    fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
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
            failed: AtomicBool::new(false),
            frozen: AtomicBool::new(false),
        }
    }
}

pub struct SourceWorkerTimingProbe {
    workers: [WorkerTimingRecord; SOURCE_WORKER_COUNT],
    coordinator: CoordinatorTimingRecord,
    cpu_sampler: Option<SourceWorkerCpuSampler>,
}

impl SourceWorkerTimingProbe {
    pub fn new(cpu_sampler: Option<SourceWorkerCpuSampler>) -> Self {
        Self {
            workers: std::array::from_fn(|_| WorkerTimingRecord::new()),
            coordinator: CoordinatorTimingRecord::new(),
            cpu_sampler,
        }
    }

    pub(crate) fn worker_start(&self) -> SourceWorkerTimingStart {
        let cpu_start = self.cpu_sampler.map_or(NO_CPU, |sampler| sampler());
        SourceWorkerTimingStart {
            started_at: Instant::now(),
            cpu_start,
        }
    }

    pub(crate) fn render_start(start: SourceWorkerTimingStart) -> Instant {
        start.started_at
    }

    pub(crate) fn record_worker(
        &self,
        parity: usize,
        sequence: u64,
        start: SourceWorkerTimingStart,
        render_duration_ns: u64,
        dispatch_started_at: Option<Instant>,
    ) {
        if !self.accepts_with_frozen(sequence, true) {
            return;
        }
        let Some(record) = self.workers.get(parity) else {
            return;
        };
        record.sequence.store(sequence, Ordering::Relaxed);
        record
            .render_ns
            .store(render_duration_ns, Ordering::Relaxed);
        record.dispatch_to_finish_ns.store(
            dispatch_started_at.map_or(0, |started| duration_ns(started.elapsed())),
            Ordering::Relaxed,
        );
        record.cpu_start.store(start.cpu_start, Ordering::Relaxed);
        record.cpu_end.store(
            self.cpu_sampler.map_or(NO_CPU, |sampler| sampler()),
            Ordering::Relaxed,
        );
        record.finished.store(true, Ordering::Release);
    }

    pub(crate) fn begin_sequence(&self, sequence: u64, deadline: Duration) {
        if self.coordinator.frozen.load(Ordering::Acquire) {
            return;
        }
        for worker in &self.workers {
            worker.finished.store(false, Ordering::Release);
            worker.sequence.store(sequence, Ordering::Relaxed);
        }
        self.coordinator.sequence.store(sequence, Ordering::Relaxed);
        self.coordinator
            .dispatch_to_deadline_start_valid
            .store(false, Ordering::Relaxed);
        self.coordinator
            .deadline_ns
            .store(duration_ns(deadline), Ordering::Relaxed);
        self.coordinator
            .dispatch_to_deadline_elapsed_valid
            .store(false, Ordering::Relaxed);
        self.coordinator.in_flight_mask.store(0, Ordering::Relaxed);
        self.coordinator.completed_mask.store(0, Ordering::Relaxed);
        self.coordinator
            .first_parity_valid
            .store(false, Ordering::Relaxed);
        self.coordinator
            .dispatch_to_first_valid
            .store(false, Ordering::Relaxed);
        self.coordinator
            .dispatch_to_both_valid
            .store(false, Ordering::Relaxed);
        self.coordinator
            .reduction_valid
            .store(false, Ordering::Relaxed);
        self.coordinator
            .coordinator_remainder_valid
            .store(false, Ordering::Relaxed);
        self.coordinator
            .engine_block_total_state
            .store(TIMING_UNSET, Ordering::Relaxed);
        self.coordinator
            .callback_total_state
            .store(TIMING_UNSET, Ordering::Relaxed);
        self.coordinator.failed.store(false, Ordering::Relaxed);
        self.coordinator
            .sequence_valid
            .store(true, Ordering::Release);
    }

    pub(crate) fn record_dispatch(&self, sequence: u64, in_flight_mask: u8) {
        if self.accepts(sequence) {
            self.coordinator
                .in_flight_mask
                .store(in_flight_mask, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_bus_dispatch(&self, sequence: u64) {
        if self.accepts(sequence) {
            self.coordinator
                .in_flight_mask
                .store(0b11, Ordering::Relaxed);
            self.coordinator
                .first_parity_valid
                .store(false, Ordering::Relaxed);
            self.coordinator
                .dispatch_to_first_valid
                .store(false, Ordering::Relaxed);
            self.coordinator
                .dispatch_to_both_valid
                .store(false, Ordering::Relaxed);
            self.coordinator.completed_mask.store(0, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_dispatch_to_deadline_start(&self, sequence: u64, elapsed: Duration) {
        self.record_timing(
            sequence,
            &self.coordinator.dispatch_to_deadline_start_ns,
            &self.coordinator.dispatch_to_deadline_start_valid,
            elapsed,
        );
    }

    pub(crate) fn record_remaining_deadline(&self, sequence: u64, remaining: Duration) {
        if self.accepts(sequence) {
            self.coordinator
                .deadline_ns
                .store(duration_ns(remaining), Ordering::Relaxed);
        }
    }

    pub(crate) fn record_reduction(&self, sequence: u64, elapsed: Duration) {
        self.record_timing(
            sequence,
            &self.coordinator.reduction_ns,
            &self.coordinator.reduction_valid,
            elapsed,
        );
    }

    pub(crate) fn record_coordinator_remainder(&self, sequence: u64, elapsed: Duration) {
        self.record_timing(
            sequence,
            &self.coordinator.coordinator_remainder_ns,
            &self.coordinator.coordinator_remainder_valid,
            elapsed,
        );
    }

    pub(crate) fn record_engine_block_total(&self, sequence: u64, elapsed: Duration) {
        self.record_total(
            sequence,
            &self.coordinator.engine_block_total_ns,
            &self.coordinator.engine_block_total_state,
            elapsed,
        );
    }

    pub fn record_callback_total(&self, elapsed: Duration) {
        let Some(sequence) = self.current_sequence() else {
            return;
        };
        self.record_total(
            sequence,
            &self.coordinator.callback_total_ns,
            &self.coordinator.callback_total_state,
            elapsed,
        );
    }

    pub(crate) fn freeze(
        &self,
        in_flight_mask: u8,
        completed_mask: u8,
        dispatch_to_deadline_elapsed: Option<Duration>,
        failed: bool,
    ) {
        if self.coordinator.frozen.load(Ordering::Acquire) {
            return;
        }
        self.coordinator
            .in_flight_mask
            .store(in_flight_mask, Ordering::Relaxed);
        self.coordinator
            .completed_mask
            .store(completed_mask, Ordering::Relaxed);
        if let Some(elapsed) = dispatch_to_deadline_elapsed {
            self.coordinator
                .dispatch_to_deadline_elapsed_ns
                .store(duration_ns(elapsed), Ordering::Relaxed);
            self.coordinator
                .dispatch_to_deadline_elapsed_valid
                .store(true, Ordering::Relaxed);
        }
        self.coordinator.failed.store(failed, Ordering::Relaxed);
        self.coordinator.frozen.store(true, Ordering::Release);
    }

    pub fn freeze_unexecuted(&self) {
        if !self.coordinator.sequence_valid.load(Ordering::Acquire) {
            self.coordinator.frozen.store(true, Ordering::Release);
        }
    }

    fn current_sequence(&self) -> Option<u64> {
        self.coordinator
            .sequence_valid
            .load(Ordering::Acquire)
            .then(|| self.coordinator.sequence.load(Ordering::Relaxed))
    }

    fn accepts(&self, sequence: u64) -> bool {
        self.accepts_with_frozen(sequence, false)
    }

    fn accepts_with_frozen(&self, sequence: u64, allow_frozen: bool) -> bool {
        self.coordinator.sequence_valid.load(Ordering::Acquire)
            && (allow_frozen || !self.coordinator.frozen.load(Ordering::Acquire))
            && self.coordinator.sequence.load(Ordering::Relaxed) == sequence
    }

    fn record_timing(
        &self,
        sequence: u64,
        value: &AtomicU64,
        valid: &AtomicBool,
        elapsed: Duration,
    ) {
        if !self.accepts(sequence) {
            return;
        }
        value.store(duration_ns(elapsed), Ordering::Relaxed);
        valid.store(true, Ordering::Relaxed);
    }

    fn record_total(&self, sequence: u64, value: &AtomicU64, state: &AtomicU8, elapsed: Duration) {
        if !self.accepts_with_frozen(sequence, true) {
            return;
        }
        if state
            .compare_exchange(
                TIMING_UNSET,
                TIMING_WRITING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            value.store(duration_ns(elapsed), Ordering::Relaxed);
            state.store(TIMING_SET, Ordering::Release);
        }
    }
}

fn duration_ns(duration: Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

#[path = "source_worker_timing_recovery.rs"]
mod recovery;
#[path = "source_worker_timing_snapshot.rs"]
mod snapshot;
pub use snapshot::{
    SourceWorkerCoordinatorTimingSnapshot, SourceWorkerTimingSnapshot,
    SourceWorkerWorkerTimingSnapshot,
};
