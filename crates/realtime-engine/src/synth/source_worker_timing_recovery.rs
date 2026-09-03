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
        if !self.accepts_with_frozen(sequence, true) {
            return;
        }
        let Some(record) = self.workers.get(parity) else {
            return;
        };
        if record.sequence.load(Ordering::Relaxed) != sequence
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
        if parity >= SOURCE_WORKER_COUNT || !self.accepts_with_frozen(sequence, allow_frozen) {
            return;
        }
        let worker_mask = 1 << parity;
        let completed = self.coordinator.completed_mask.load(Ordering::Relaxed);
        if completed & worker_mask != 0 {
            return;
        }
        if self
            .coordinator
            .first_parity_valid
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            self.coordinator
                .first_parity
                .store(parity as u8, Ordering::Relaxed);
            self.coordinator
                .dispatch_to_first_ns
                .store(duration_ns(dispatch_to_completion), Ordering::Relaxed);
            self.coordinator
                .dispatch_to_first_valid
                .store(true, Ordering::Relaxed);
        }
        let completed = completed | worker_mask;
        self.coordinator
            .completed_mask
            .store(completed, Ordering::Relaxed);
        if completed == 0b11 {
            self.coordinator
                .dispatch_to_both_ns
                .store(duration_ns(dispatch_to_completion), Ordering::Relaxed);
            self.coordinator
                .dispatch_to_both_valid
                .store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_bus_completion(
        &self,
        sequence: u64,
        parity: usize,
        dispatch_to_completion: Duration,
    ) {
        self.record_bus_completion_inner(sequence, parity, dispatch_to_completion, false);
    }

    pub(crate) fn record_recovery_bus_completion(
        &self,
        sequence: u64,
        parity: usize,
        dispatch_to_completion: Duration,
    ) {
        self.record_bus_completion_inner(sequence, parity, dispatch_to_completion, true);
    }

    fn record_bus_completion_inner(
        &self,
        sequence: u64,
        parity: usize,
        dispatch_to_completion: Duration,
        allow_frozen: bool,
    ) {
        if parity >= SOURCE_WORKER_COUNT || !self.accepts_with_frozen(sequence, allow_frozen) {
            return;
        }
        let worker_mask = 1 << parity;
        let completed = self.coordinator.completed_mask.load(Ordering::Relaxed);
        if completed & worker_mask != 0 {
            return;
        }
        let completed = completed | worker_mask;
        self.coordinator
            .completed_mask
            .store(completed, Ordering::Relaxed);
        if completed == 0b11 {
            self.coordinator
                .dispatch_to_both_ns
                .store(duration_ns(dispatch_to_completion), Ordering::Relaxed);
            self.coordinator
                .dispatch_to_both_valid
                .store(true, Ordering::Relaxed);
        }
    }
}
