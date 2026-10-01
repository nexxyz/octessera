use super::records::SequenceTimingRecord;
use super::{SourceWorkerTimingProbe, NO_CPU, SOURCE_WORKER_COUNT, TIMING_SET};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

impl SourceWorkerTimingProbe {
    pub(super) fn latest_completed_record(&self) -> Option<&SequenceTimingRecord> {
        self.newest_record(|record| record.coordinator.fully_completed.load(Ordering::Acquire))
    }

    fn selected_record_for_snapshot(&self) -> Option<&SequenceTimingRecord> {
        if self.failed_sequence_valid.load(Ordering::Acquire) {
            return self.failed_sequence().and_then(|sequence| {
                self.record_for(sequence)
                    .filter(|record| self.accepts_record(record, sequence, true))
            });
        }
        self.latest_completed_record().or_else(|| {
            if self.unexecuted_frozen() {
                None
            } else {
                self.newest_record(|_| true)
            }
        })
    }

    pub fn snapshot(&self) -> SourceWorkerTimingSnapshot {
        let Some(record) = self.selected_record_for_snapshot() else {
            return empty_snapshot(self.unexecuted_frozen());
        };
        let coordinator_record = &record.coordinator;
        let sequence = coordinator_record
            .sequence_valid
            .load(Ordering::Acquire)
            .then(|| coordinator_record.sequence.load(Ordering::Relaxed));
        let workers = record.workers.each_ref().map(|worker| {
            let finished = worker.finished.load(Ordering::Acquire)
                && sequence
                    .is_some_and(|sequence| worker.sequence.load(Ordering::Acquire) == sequence);
            SourceWorkerWorkerTimingSnapshot {
                sequence: finished.then_some(sequence).flatten(),
                render_ns: finished.then(|| worker.render_ns.load(Ordering::Relaxed)),
                dispatch_to_finish_ns: finished
                    .then(|| worker.dispatch_to_finish_ns.load(Ordering::Relaxed)),
                cpu_start: cpu_value(finished, worker.cpu_start.load(Ordering::Relaxed)),
                cpu_end: cpu_value(finished, worker.cpu_end.load(Ordering::Relaxed)),
                finished,
            }
        });
        let coordinator = SourceWorkerCoordinatorTimingSnapshot {
            sequence,
            deadline_ns: sequence.map(|_| coordinator_record.deadline_ns.load(Ordering::Relaxed)),
            dispatch_to_deadline_start_ns: timing_value(
                &coordinator_record.dispatch_to_deadline_start_ns,
                &coordinator_record.dispatch_to_deadline_start_valid,
            ),
            dispatch_to_deadline_elapsed_ns: timing_value(
                &coordinator_record.dispatch_to_deadline_elapsed_ns,
                &coordinator_record.dispatch_to_deadline_elapsed_valid,
            ),
            in_flight_mask: sequence
                .map(|_| coordinator_record.in_flight_mask.load(Ordering::Relaxed)),
            completed_mask: sequence
                .map(|_| coordinator_record.completed_mask.load(Ordering::Relaxed)),
            first_parity: if coordinator_record
                .first_parity_valid
                .load(Ordering::Acquire)
            {
                Some(coordinator_record.first_parity.load(Ordering::Relaxed) as usize)
            } else {
                None
            },
            dispatch_to_first_ns: timing_value(
                &coordinator_record.dispatch_to_first_ns,
                &coordinator_record.dispatch_to_first_valid,
            ),
            dispatch_to_both_ns: timing_value(
                &coordinator_record.dispatch_to_both_ns,
                &coordinator_record.dispatch_to_both_valid,
            ),
            reduction_ns: timing_value(
                &coordinator_record.reduction_ns,
                &coordinator_record.reduction_valid,
            ),
            coordinator_remainder_ns: timing_value(
                &coordinator_record.coordinator_remainder_ns,
                &coordinator_record.coordinator_remainder_valid,
            ),
            engine_block_total_ns: total_value(
                &coordinator_record.engine_block_total_ns,
                &coordinator_record.engine_block_total_state,
            ),
            callback_total_ns: total_value(
                &coordinator_record.callback_total_ns,
                &coordinator_record.callback_total_state,
            ),
            failed: coordinator_record.failed.load(Ordering::Relaxed),
            frozen: coordinator_record.frozen.load(Ordering::Acquire),
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

    fn newest_record(
        &self,
        matches: impl Fn(&SequenceTimingRecord) -> bool,
    ) -> Option<&SequenceTimingRecord> {
        let mut selected = None;
        for record in &self.records {
            if !record.coordinator.sequence_valid.load(Ordering::Acquire) || !matches(record) {
                continue;
            }
            selected = match selected {
                Some(current)
                    if !is_newer_sequence(record_sequence(record), record_sequence(current)) =>
                {
                    Some(current)
                }
                _ => Some(record),
            };
        }
        selected
    }

    fn unexecuted_frozen(&self) -> bool {
        self.unexecuted_frozen.load(Ordering::Acquire)
    }
}

fn record_sequence(record: &SequenceTimingRecord) -> u64 {
    record.coordinator.sequence.load(Ordering::Relaxed)
}

fn is_newer_sequence(candidate: u64, current: u64) -> bool {
    candidate != current && candidate.wrapping_sub(current) < (1_u64 << 63)
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

fn empty_snapshot(frozen: bool) -> SourceWorkerTimingSnapshot {
    SourceWorkerTimingSnapshot {
        workers: std::array::from_fn(|_| SourceWorkerWorkerTimingSnapshot {
            sequence: None,
            render_ns: None,
            dispatch_to_finish_ns: None,
            cpu_start: None,
            cpu_end: None,
            finished: false,
        }),
        coordinator: SourceWorkerCoordinatorTimingSnapshot {
            sequence: None,
            deadline_ns: None,
            dispatch_to_deadline_start_ns: None,
            dispatch_to_deadline_elapsed_ns: None,
            in_flight_mask: None,
            completed_mask: None,
            first_parity: None,
            dispatch_to_first_ns: None,
            dispatch_to_both_ns: None,
            reduction_ns: None,
            coordinator_remainder_ns: None,
            engine_block_total_ns: None,
            callback_total_ns: None,
            failed: false,
            frozen,
        },
        late_after_deadline_ns: None,
        cpu_endpoint_changed: false,
    }
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
