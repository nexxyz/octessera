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
            if failed {
                if let Some(stamp) = self.expected_stamp {
                    probe.freeze_sequence(
                        stamp.quantum_sequence,
                        self.in_flight_mask,
                        self.completed_mask,
                        dispatch_to_deadline_elapsed,
                        true,
                    );
                } else {
                    probe.freeze_unexecuted();
                }
            } else {
                probe.freeze_latest_completed();
            }
        }
    }

    pub fn timing_block_start(&self) -> Option<Instant> {
        self.timing_probe.as_ref().map(|_| Instant::now())
    }

    pub fn record_engine_block_total(&self, started_at: Option<Instant>) {
        let sequence = self
            .timing_output_sequence
            .or_else(|| self.expected_stamp.map(|stamp| stamp.quantum_sequence));
        if let (Some(probe), Some(started_at), Some(sequence)) =
            (self.timing_probe.as_ref(), started_at, sequence)
        {
            probe.record_engine_block_total(sequence, started_at.elapsed());
        }
    }

    pub(crate) fn record_output_sequence(&mut self, sequence: u64) {
        self.timing_output_sequence = Some(sequence);
        if let Some(probe) = self.timing_probe.as_ref() {
            probe.record_output_sequence(sequence);
        }
    }

    pub(crate) fn take_coordinator_remainder_started_at(&mut self) -> Option<Instant> {
        self.coordinator_remainder_started_at.take()
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(crate) fn take_routing_coordinator_remainder_started_at(
        &mut self,
    ) -> Option<(u64, Instant)> {
        self.routing_coordinator_remainder_started_at.take()
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

    #[cfg(feature = "routing-tree-benchmark")]
    pub(crate) fn record_routing_coordinator_remainder(&self, timing: Option<(u64, Instant)>) {
        if let (Some(probe), Some((sequence, started_at))) = (self.timing_probe.as_ref(), timing) {
            probe.record_coordinator_remainder(sequence, started_at.elapsed());
        }
    }
}
