use super::super::super::source_worker_timing::SourceWorkerTimingProbe;
use super::SourceWorkerRuntime;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl SourceWorkerRuntime {
    pub fn attach_timing_probe(&mut self, probe: Arc<SourceWorkerTimingProbe>) {
        self.timing_probe = Some(probe);
    }

    pub(super) fn freeze_timing(
        &self,
        failed: bool,
        dispatch_to_deadline_elapsed: Option<Duration>,
    ) {
        if let Some(probe) = self.timing_probe.as_ref() {
            let completed_mask = if !failed
                && self.expected_stamp.is_some()
                && self.in_flight_mask == 0
                && self.completed_mask == 0
            {
                0b11
            } else {
                self.completed_mask
            };
            probe.freeze(
                self.in_flight_mask,
                completed_mask,
                dispatch_to_deadline_elapsed,
                failed,
            );
        }
    }

    pub fn timing_block_start(&self) -> Option<Instant> {
        self.timing_probe.as_ref().map(|_| Instant::now())
    }

    pub fn record_engine_block_total(&self, started_at: Option<Instant>) {
        if let (Some(probe), Some(started_at), Some(stamp)) =
            (self.timing_probe.as_ref(), started_at, self.expected_stamp)
        {
            probe.record_engine_block_total(stamp.quantum_sequence, started_at.elapsed());
        }
    }

    pub(crate) fn take_coordinator_remainder_started_at(&mut self) -> Option<Instant> {
        self.coordinator_remainder_started_at.take()
    }

    pub(super) fn record_dispatch_to_deadline_start(
        &self,
        deadline_start: Instant,
        deadline: Instant,
    ) {
        if let (Some(probe), Some(dispatch_started_at), Some(stamp)) = (
            self.timing_probe.as_ref(),
            self.dispatch_started_at,
            self.expected_stamp,
        ) {
            probe.record_remaining_deadline(
                stamp.quantum_sequence,
                deadline.saturating_duration_since(deadline_start),
            );
            probe.record_dispatch_to_deadline_start(
                stamp.quantum_sequence,
                deadline_start.duration_since(dispatch_started_at),
            );
        }
    }

    pub(super) fn dispatch_elapsed(&self) -> Option<Duration> {
        self.dispatch_started_at
            .map(|started_at| started_at.elapsed())
    }

    pub(crate) fn record_coordinator_remainder(&self, started_at: Option<Instant>) {
        if let (Some(probe), Some(started_at), Some(stamp)) =
            (self.timing_probe.as_ref(), started_at, self.expected_stamp)
        {
            probe.record_coordinator_remainder(stamp.quantum_sequence, started_at.elapsed());
        }
    }
}
