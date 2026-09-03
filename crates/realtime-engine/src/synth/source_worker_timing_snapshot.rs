use super::{SourceWorkerTimingProbe, NO_CPU, SOURCE_WORKER_COUNT, TIMING_SET};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

impl SourceWorkerTimingProbe {
    pub fn snapshot(&self) -> SourceWorkerTimingSnapshot {
        let coordinator_frozen = self.coordinator.frozen.load(Ordering::Acquire);
        let sequence = self
            .coordinator
            .sequence_valid
            .load(Ordering::Acquire)
            .then(|| self.coordinator.sequence.load(Ordering::Relaxed));
        let workers = self.workers.each_ref().map(|record| {
            let finished = record.finished.load(Ordering::Acquire);
            SourceWorkerWorkerTimingSnapshot {
                sequence: finished.then(|| record.sequence.load(Ordering::Relaxed)),
                render_ns: finished.then(|| record.render_ns.load(Ordering::Relaxed)),
                dispatch_to_finish_ns: finished
                    .then(|| record.dispatch_to_finish_ns.load(Ordering::Relaxed)),
                cpu_start: cpu_value(finished, record.cpu_start.load(Ordering::Relaxed)),
                cpu_end: cpu_value(finished, record.cpu_end.load(Ordering::Relaxed)),
                finished,
            }
        });
        let dispatch_to_deadline_start_ns = timing_value(
            &self.coordinator.dispatch_to_deadline_start_ns,
            &self.coordinator.dispatch_to_deadline_start_valid,
        );
        let coordinator = SourceWorkerCoordinatorTimingSnapshot {
            sequence,
            deadline_ns: sequence.map(|_| self.coordinator.deadline_ns.load(Ordering::Relaxed)),
            dispatch_to_deadline_start_ns,
            dispatch_to_deadline_elapsed_ns: if self
                .coordinator
                .dispatch_to_deadline_elapsed_valid
                .load(Ordering::Relaxed)
            {
                Some(
                    self.coordinator
                        .dispatch_to_deadline_elapsed_ns
                        .load(Ordering::Relaxed),
                )
            } else {
                None
            },
            in_flight_mask: sequence
                .map(|_| self.coordinator.in_flight_mask.load(Ordering::Relaxed)),
            completed_mask: sequence
                .map(|_| self.coordinator.completed_mask.load(Ordering::Relaxed)),
            first_parity: if self.coordinator.first_parity_valid.load(Ordering::Acquire) {
                Some(self.coordinator.first_parity.load(Ordering::Relaxed) as usize)
            } else {
                None
            },
            dispatch_to_first_ns: timing_value(
                &self.coordinator.dispatch_to_first_ns,
                &self.coordinator.dispatch_to_first_valid,
            ),
            dispatch_to_both_ns: timing_value(
                &self.coordinator.dispatch_to_both_ns,
                &self.coordinator.dispatch_to_both_valid,
            ),
            reduction_ns: timing_value(
                &self.coordinator.reduction_ns,
                &self.coordinator.reduction_valid,
            ),
            coordinator_remainder_ns: timing_value(
                &self.coordinator.coordinator_remainder_ns,
                &self.coordinator.coordinator_remainder_valid,
            ),
            engine_block_total_ns: total_value(
                &self.coordinator.engine_block_total_ns,
                &self.coordinator.engine_block_total_state,
            ),
            callback_total_ns: total_value(
                &self.coordinator.callback_total_ns,
                &self.coordinator.callback_total_state,
            ),
            failed: self.coordinator.failed.load(Ordering::Relaxed),
            frozen: coordinator_frozen,
        };
        let deadline_boundary = coordinator
            .dispatch_to_deadline_start_ns
            .zip(coordinator.deadline_ns)
            .and_then(|(start, deadline)| start.checked_add(deadline));
        let late_after_deadline_ns = deadline_boundary.and_then(|deadline| {
            workers
                .iter()
                .filter_map(|worker| {
                    (worker.sequence == coordinator.sequence)
                        .then_some(worker.dispatch_to_finish_ns)
                        .flatten()
                        .filter(|finish| *finish > deadline)
                        .map(|finish| finish - deadline)
                })
                .max()
        });
        let cpu_endpoint_changed = workers.iter().any(|worker| {
            worker.cpu_start.is_some()
                && worker.cpu_end.is_some()
                && worker.cpu_start != worker.cpu_end
        });
        SourceWorkerTimingSnapshot {
            workers,
            coordinator,
            late_after_deadline_ns,
            cpu_endpoint_changed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceWorkerWorkerTimingSnapshot {
    pub sequence: Option<u64>,
    pub render_ns: Option<u64>,
    pub dispatch_to_finish_ns: Option<u64>,
    pub cpu_start: Option<u32>,
    pub cpu_end: Option<u32>,
    pub finished: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceWorkerCoordinatorTimingSnapshot {
    pub sequence: Option<u64>,
    pub deadline_ns: Option<u64>,
    pub dispatch_to_deadline_start_ns: Option<u64>,
    pub dispatch_to_deadline_elapsed_ns: Option<u64>,
    pub in_flight_mask: Option<u8>,
    pub completed_mask: Option<u8>,
    pub first_parity: Option<usize>,
    pub dispatch_to_first_ns: Option<u64>,
    pub dispatch_to_both_ns: Option<u64>,
    pub reduction_ns: Option<u64>,
    pub coordinator_remainder_ns: Option<u64>,
    pub engine_block_total_ns: Option<u64>,
    pub callback_total_ns: Option<u64>,
    pub failed: bool,
    pub frozen: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceWorkerTimingSnapshot {
    pub workers: [SourceWorkerWorkerTimingSnapshot; SOURCE_WORKER_COUNT],
    pub coordinator: SourceWorkerCoordinatorTimingSnapshot,
    pub late_after_deadline_ns: Option<u64>,
    pub cpu_endpoint_changed: bool,
}

fn cpu_value(finished: bool, value: u32) -> Option<u32> {
    finished.then_some(value).filter(|cpu| *cpu != NO_CPU)
}

fn timing_value(value: &AtomicU64, valid: &AtomicBool) -> Option<u64> {
    valid
        .load(Ordering::Acquire)
        .then(|| value.load(Ordering::Relaxed))
}

fn total_value(value: &AtomicU64, state: &std::sync::atomic::AtomicU8) -> Option<u64> {
    (state.load(Ordering::Acquire) == TIMING_SET).then(|| value.load(Ordering::Relaxed))
}
