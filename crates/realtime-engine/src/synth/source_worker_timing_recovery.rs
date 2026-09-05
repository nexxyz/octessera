use super::{duration_ns, SourceWorkerTimingProbe, SOURCE_WORKER_COUNT};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

impl SourceWorkerTimingProbe {
    pub(crate) fn record_bus_worker(
        &self,
        parity: usize,
        sequence: u64,
        render_duration_ns: u64,
        dispatch_started_at: Option<Instant>,
    ) {
        let Some(sequence_record) = self.record_for(sequence) else {
            return;
        };
        if parity >= SOURCE_WORKER_COUNT || !self.accepts_record(sequence_record, sequence, true) {
            return;
        }
        let record = &sequence_record.workers[parity];
        if record.sequence.load(Ordering::Acquire) != sequence
            || !record.finished.load(Ordering::Acquire)
        {
            return;
        }
        record
            .render_ns
            .fetch_add(render_duration_ns, Ordering::Relaxed);
        record.dispatch_to_finish_ns.store(
            dispatch_started_at.map_or(0, |started| duration_ns(started.elapsed())),
            Ordering::Relaxed,
        );
        record.cpu_end.store(
            self.cpu_sampler.map_or(super::NO_CPU, |sampler| sampler()),
            Ordering::Relaxed,
        );
    }

    pub(crate) fn record_completion(
        &self,
        sequence: u64,
        parity: usize,
        dispatch_to_completion: Duration,
    ) {
        self.record_completion_inner(sequence, parity, dispatch_to_completion, false);
    }

    pub(crate) fn record_recovery_completion(
        &self,
        sequence: u64,
        parity: usize,
        dispatch_to_completion: Duration,
    ) {
        self.record_completion_inner(sequence, parity, dispatch_to_completion, true);
    }

    fn record_completion_inner(
        &self,
        sequence: u64,
        parity: usize,
        dispatch_to_completion: Duration,
        allow_frozen: bool,
    ) {
        let Some(record) = self.record_for(sequence) else {
            return;
        };
        if parity >= SOURCE_WORKER_COUNT || !self.accepts_record(record, sequence, allow_frozen) {
            return;
        }
        let worker_mask = 1 << parity;
        let coordinator = &record.coordinator;
        let completed = coordinator.completed_mask.load(Ordering::Relaxed);
        if completed & worker_mask != 0 {
            return;
        }
        if completed == 0 {
            coordinator
                .first_parity
                .store(parity as u8, Ordering::Relaxed);
            coordinator
                .dispatch_to_first_ns
                .store(duration_ns(dispatch_to_completion), Ordering::Relaxed);
            coordinator
                .first_parity_valid
                .store(true, Ordering::Release);
            coordinator
                .dispatch_to_first_valid
                .store(true, Ordering::Release);
        }
        let completed = completed | worker_mask;
        coordinator
            .in_flight_mask
            .fetch_and(!worker_mask, Ordering::Relaxed);
        coordinator
            .completed_mask
            .store(completed, Ordering::Relaxed);
        if completed == 0b11 {
            coordinator
                .dispatch_to_both_ns
                .store(duration_ns(dispatch_to_completion), Ordering::Relaxed);
            coordinator
                .dispatch_to_both_valid
                .store(true, Ordering::Release);
        }
    }

    pub(crate) fn record_bus_completion(
        &self,
        sequence: u64,
        parity: usize,
        dispatch_to_completion: Duration,
    ) {
        self.record_completion_inner(sequence, parity, dispatch_to_completion, false);
    }

    pub(crate) fn record_recovery_bus_completion(
        &self,
        sequence: u64,
        parity: usize,
        dispatch_to_completion: Duration,
    ) {
        self.record_completion_inner(sequence, parity, dispatch_to_completion, true);
    }
}
