use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::time::{Duration, Instant};

const SOURCE_WORKER_COUNT: usize = 2;
const TIMING_RECORD_COUNT: usize = 2;
const NO_CPU: u32 = u32::MAX;
const NO_SEQUENCE: u64 = u64::MAX;
const TIMING_UNSET: u8 = 0;
const TIMING_WRITING: u8 = 1;
const TIMING_SET: u8 = 2;

pub type SourceWorkerCpuSampler = fn() -> u32;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceWorkerTimingStart {
    started_at: Instant,
    cpu_start: u32,
}

#[path = "source_worker_timing_records.rs"]
mod records;
use records::SequenceTimingRecord;

pub struct SourceWorkerTimingProbe {
    records: [SequenceTimingRecord; TIMING_RECORD_COUNT],
    latest_sequence: AtomicU64,
    latest_sequence_valid: AtomicBool,
    latest_output_sequence: AtomicU64,
    latest_output_sequence_valid: AtomicBool,
    failed_sequence: AtomicU64,
    failed_sequence_valid: AtomicBool,
    probe_frozen: AtomicBool,
    unexecuted_frozen: AtomicBool,
    cpu_sampler: Option<SourceWorkerCpuSampler>,
}

impl SourceWorkerTimingProbe {
    pub fn new(cpu_sampler: Option<SourceWorkerCpuSampler>) -> Self {
        Self {
            records: std::array::from_fn(|_| SequenceTimingRecord::new()),
            latest_sequence: AtomicU64::new(NO_SEQUENCE),
            latest_sequence_valid: AtomicBool::new(false),
            latest_output_sequence: AtomicU64::new(NO_SEQUENCE),
            latest_output_sequence_valid: AtomicBool::new(false),
            failed_sequence: AtomicU64::new(0),
            failed_sequence_valid: AtomicBool::new(false),
            probe_frozen: AtomicBool::new(false),
            unexecuted_frozen: AtomicBool::new(false),
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
        let Some(record) = self.record_for(sequence) else {
            return;
        };
        if parity >= SOURCE_WORKER_COUNT || !self.accepts_record(record, sequence, true) {
            return;
        }
        let worker = &record.workers[parity];
        worker
            .render_ns
            .store(render_duration_ns, Ordering::Relaxed);
        worker.dispatch_to_finish_ns.store(
            dispatch_started_at.map_or(0, |started| duration_ns(started.elapsed())),
            Ordering::Relaxed,
        );
        worker.cpu_start.store(start.cpu_start, Ordering::Relaxed);
        worker.cpu_end.store(
            self.cpu_sampler.map_or(NO_CPU, |sampler| sampler()),
            Ordering::Relaxed,
        );
        worker.sequence.store(sequence, Ordering::Relaxed);
        worker.finished.store(true, Ordering::Release);
    }

    pub(crate) fn begin_sequence(&self, sequence: u64, deadline: Duration) -> bool {
        if self.probe_frozen.load(Ordering::Acquire) {
            return false;
        }
        let record = self.record_for(sequence).expect("timing record slot");
        record
            .coordinator
            .reset(sequence, deadline, &record.workers);
        self.latest_sequence.store(sequence, Ordering::Release);
        self.latest_sequence_valid.store(true, Ordering::Release);
        true
    }

    pub(crate) fn record_dispatch(&self, sequence: u64, in_flight_mask: u8) {
        if let Some(record) = self.accepted_record(sequence) {
            record
                .coordinator
                .in_flight_mask
                .store(in_flight_mask, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_bus_dispatch(&self, sequence: u64) {
        if let Some(record) = self.accepted_record(sequence) {
            record
                .coordinator
                .in_flight_mask
                .store(0b11, Ordering::Relaxed);
            record
                .coordinator
                .first_parity_valid
                .store(false, Ordering::Relaxed);
            record
                .coordinator
                .dispatch_to_first_valid
                .store(false, Ordering::Relaxed);
            record
                .coordinator
                .dispatch_to_both_valid
                .store(false, Ordering::Relaxed);
            record
                .coordinator
                .completed_mask
                .store(0, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_dispatch_to_deadline_start(&self, sequence: u64, elapsed: Duration) {
        if let Some(record) = self.accepted_record(sequence) {
            self.record_timing(
                record,
                sequence,
                &record.coordinator.dispatch_to_deadline_start_ns,
                &record.coordinator.dispatch_to_deadline_start_valid,
                elapsed,
            );
        }
    }

    pub(crate) fn record_remaining_deadline(&self, sequence: u64, remaining: Duration) {
        if let Some(record) = self.accepted_record(sequence) {
            record
                .coordinator
                .deadline_ns
                .store(duration_ns(remaining), Ordering::Relaxed);
        }
    }

    pub(crate) fn record_reduction(&self, sequence: u64, elapsed: Duration) {
        if let Some(record) = self.accepted_record(sequence) {
            self.record_timing(
                record,
                sequence,
                &record.coordinator.reduction_ns,
                &record.coordinator.reduction_valid,
                elapsed,
            );
            self.mark_completed_if_ready(record, sequence);
        }
    }

    pub(crate) fn record_coordinator_remainder(&self, sequence: u64, elapsed: Duration) {
        if let Some(record) = self.accepted_record(sequence) {
            self.record_timing(
                record,
                sequence,
                &record.coordinator.coordinator_remainder_ns,
                &record.coordinator.coordinator_remainder_valid,
                elapsed,
            );
            self.mark_completed_if_ready(record, sequence);
        }
    }

    pub(crate) fn record_engine_block_total(&self, sequence: u64, elapsed: Duration) {
        if let Some(record) = self.accepted_record_with_frozen(sequence) {
            self.record_total(
                record,
                sequence,
                &record.coordinator.engine_block_total_ns,
                &record.coordinator.engine_block_total_state,
                elapsed,
            );
            self.mark_completed_if_ready(record, sequence);
        }
    }

    pub(crate) fn record_output_sequence(&self, sequence: u64) {
        if self.failed_sequence_valid.load(Ordering::Acquire) {
            return;
        }
        if self.accepted_record(sequence).is_some() {
            self.latest_output_sequence
                .store(sequence, Ordering::Release);
            self.latest_output_sequence_valid
                .store(true, Ordering::Release);
        }
    }

    pub fn record_callback_total(&self, elapsed: Duration) {
        let Some(sequence) = self.callback_sequence() else {
            return;
        };
        self.record_callback_total_for_sequence(sequence, elapsed);
    }

    pub(crate) fn record_callback_total_for_sequence(&self, sequence: u64, elapsed: Duration) {
        if self
            .failed_sequence()
            .is_some_and(|failed_sequence| failed_sequence != sequence)
        {
            return;
        }
        if let Some(record) = self.accepted_record_with_frozen(sequence) {
            self.record_total(
                record,
                sequence,
                &record.coordinator.callback_total_ns,
                &record.coordinator.callback_total_state,
                elapsed,
            );
            self.mark_completed_if_ready(record, sequence);
        }
    }

    #[cfg(test)]
    pub(crate) fn freeze(
        &self,
        in_flight_mask: u8,
        completed_mask: u8,
        dispatch_to_deadline_elapsed: Option<Duration>,
        failed: bool,
    ) {
        if failed {
            if let Some(sequence) = self.current_sequence() {
                self.freeze_sequence(
                    sequence,
                    in_flight_mask,
                    completed_mask,
                    dispatch_to_deadline_elapsed,
                    true,
                );
            } else {
                self.freeze_unexecuted();
            }
        } else {
            self.freeze_latest_completed();
        }
    }

    pub(crate) fn freeze_sequence(
        &self,
        sequence: u64,
        in_flight_mask: u8,
        completed_mask: u8,
        dispatch_to_deadline_elapsed: Option<Duration>,
        failed: bool,
    ) {
        if self.probe_frozen.load(Ordering::Acquire) {
            return;
        }
        let Some(record) = self.record_for(sequence) else {
            return;
        };
        if !self.accepts_record(record, sequence, true)
            || record.coordinator.frozen.load(Ordering::Acquire)
        {
            return;
        }
        record
            .coordinator
            .in_flight_mask
            .store(in_flight_mask, Ordering::Relaxed);
        record
            .coordinator
            .completed_mask
            .store(completed_mask, Ordering::Relaxed);
        if let Some(elapsed) = dispatch_to_deadline_elapsed {
            record
                .coordinator
                .dispatch_to_deadline_elapsed_ns
                .store(duration_ns(elapsed), Ordering::Relaxed);
            record
                .coordinator
                .dispatch_to_deadline_elapsed_valid
                .store(true, Ordering::Relaxed);
        }
        record.coordinator.failed.store(failed, Ordering::Relaxed);
        record.coordinator.frozen.store(true, Ordering::Release);
        if failed {
            self.failed_sequence.store(sequence, Ordering::Relaxed);
            self.failed_sequence_valid.store(true, Ordering::Release);
        }
        self.probe_frozen.store(true, Ordering::Release);
    }

    pub fn freeze_latest_completed(&self) {
        if self.probe_frozen.load(Ordering::Acquire) {
            return;
        }
        let Some(record) = self.latest_completed_record() else {
            self.unexecuted_frozen.store(true, Ordering::Release);
            self.probe_frozen.store(true, Ordering::Release);
            return;
        };
        let sequence = record.coordinator.sequence.load(Ordering::Relaxed);
        self.freeze_sequence(
            sequence,
            record.coordinator.in_flight_mask.load(Ordering::Relaxed),
            record.coordinator.completed_mask.load(Ordering::Relaxed),
            None,
            false,
        );
    }

    pub fn freeze_unexecuted(&self) {
        if self.probe_frozen.load(Ordering::Acquire) {
            return;
        }
        if !self
            .records
            .iter()
            .any(|record| record.coordinator.sequence_valid.load(Ordering::Acquire))
        {
            self.unexecuted_frozen.store(true, Ordering::Release);
            self.probe_frozen.store(true, Ordering::Release);
        }
    }

    fn current_sequence(&self) -> Option<u64> {
        let sequence = self.latest_sequence.load(Ordering::Acquire);
        self.latest_sequence_valid
            .load(Ordering::Acquire)
            .then_some(sequence)
    }

    fn failed_sequence(&self) -> Option<u64> {
        self.failed_sequence_valid
            .load(Ordering::Acquire)
            .then(|| self.failed_sequence.load(Ordering::Relaxed))
    }

    fn callback_sequence(&self) -> Option<u64> {
        if let Some(sequence) = self.failed_sequence() {
            return Some(sequence);
        }
        let current = self.current_sequence();
        if current.is_some_and(|sequence| {
            self.record_for(sequence).is_some_and(|record| {
                record.coordinator.frozen.load(Ordering::Acquire)
                    && record.coordinator.failed.load(Ordering::Relaxed)
            })
        }) {
            return current;
        }
        let output = self.latest_output_sequence.load(Ordering::Acquire);
        if self.latest_output_sequence_valid.load(Ordering::Acquire)
            && self.accepted_record_with_frozen(output).is_some()
        {
            return Some(output);
        }
        None
    }

    fn record_for(&self, sequence: u64) -> Option<&SequenceTimingRecord> {
        self.records.get((sequence & 1) as usize)
    }

    fn accepted_record(&self, sequence: u64) -> Option<&SequenceTimingRecord> {
        self.record_for(sequence)
            .filter(|record| self.accepts_record(record, sequence, false))
    }

    fn accepted_record_with_frozen(&self, sequence: u64) -> Option<&SequenceTimingRecord> {
        self.record_for(sequence)
            .filter(|record| self.accepts_record(record, sequence, true))
    }

    fn accepts_record(
        &self,
        record: &SequenceTimingRecord,
        sequence: u64,
        allow_frozen: bool,
    ) -> bool {
        record.coordinator.sequence_valid.load(Ordering::Acquire)
            && self
                .failed_sequence()
                .is_none_or(|failed_sequence| failed_sequence == sequence)
            && (allow_frozen || !record.coordinator.frozen.load(Ordering::Acquire))
            && record.coordinator.sequence.load(Ordering::Relaxed) == sequence
    }

    fn record_timing(
        &self,
        record: &SequenceTimingRecord,
        sequence: u64,
        value: &AtomicU64,
        valid: &AtomicBool,
        elapsed: Duration,
    ) {
        if !self.accepts_record(record, sequence, false) {
            return;
        }
        value.store(duration_ns(elapsed), Ordering::Relaxed);
        valid.store(true, Ordering::Release);
    }

    fn record_total(
        &self,
        record: &SequenceTimingRecord,
        sequence: u64,
        value: &AtomicU64,
        state: &AtomicU8,
        elapsed: Duration,
    ) {
        if !self.accepts_record(record, sequence, true) {
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

    fn mark_completed_if_ready(&self, record: &SequenceTimingRecord, sequence: u64) {
        let coordinator = &record.coordinator;
        if !self.accepts_record(record, sequence, false)
            || coordinator.in_flight_mask.load(Ordering::Relaxed) != 0
            || coordinator.completed_mask.load(Ordering::Relaxed) != 0b11
            || !coordinator.dispatch_to_both_valid.load(Ordering::Acquire)
            || !coordinator.reduction_valid.load(Ordering::Acquire)
            || !coordinator
                .coordinator_remainder_valid
                .load(Ordering::Acquire)
            || coordinator.engine_block_total_state.load(Ordering::Acquire) != TIMING_SET
            || coordinator.callback_total_state.load(Ordering::Acquire) != TIMING_SET
        {
            return;
        }
        coordinator.fully_completed.store(true, Ordering::Release);
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
