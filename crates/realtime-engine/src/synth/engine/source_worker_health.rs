#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SourceWorkerHealth {
    Disabled = 0,
    Healthy = 1,
    DeadlineMiss = 2,
    DispatchFailed = 3,
    CompletionFailed = 4,
    WorkerExited = 5,
    InvalidBlock = 6,
}

impl SourceWorkerHealth {
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Disabled,
            1 => Self::Healthy,
            2 => Self::DeadlineMiss,
            3 => Self::DispatchFailed,
            4 => Self::CompletionFailed,
            5 => Self::WorkerExited,
            6 => Self::InvalidBlock,
            _ => Self::CompletionFailed,
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::DispatchFailed | Self::CompletionFailed | Self::WorkerExited | Self::InvalidBlock
        )
    }

    pub const fn is_recovering(self) -> bool {
        matches!(self, Self::DeadlineMiss)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Healthy => "healthy",
            Self::DeadlineMiss => "deadline_miss",
            Self::DispatchFailed => "dispatch_failed",
            Self::CompletionFailed => "completion_failed",
            Self::WorkerExited => "worker_exited",
            Self::InvalidBlock => "invalid_block",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceWorkerHealthSnapshot {
    pub status: SourceWorkerHealth,
    pub failed_mask: u8,
    pub last_completion_sequence: u64,
    pub dispatch_failures: u64,
    pub completion_failures: u64,
    pub deadline_misses: u64,
    pub deadline_recoveries: u64,
    pub worker_exits: u64,
    pub invalid_blocks: u64,
}

#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
pub(super) struct SourceWorkerHealthState {
    status: AtomicU8,
    failed_mask: AtomicU8,
    last_completion_sequence: AtomicU64,
    dispatch_failures: AtomicU64,
    completion_failures: AtomicU64,
    deadline_misses: AtomicU64,
    deadline_recoveries: AtomicU64,
    worker_exits: AtomicU64,
    invalid_blocks: AtomicU64,
}

#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
impl SourceWorkerHealthState {
    pub(super) fn new(status: SourceWorkerHealth) -> Self {
        Self {
            status: AtomicU8::new(status as u8),
            failed_mask: AtomicU8::new(0),
            last_completion_sequence: AtomicU64::new(0),
            dispatch_failures: AtomicU64::new(0),
            completion_failures: AtomicU64::new(0),
            deadline_misses: AtomicU64::new(0),
            deadline_recoveries: AtomicU64::new(0),
            worker_exits: AtomicU64::new(0),
            invalid_blocks: AtomicU64::new(0),
        }
    }

    pub(super) fn status(&self) -> SourceWorkerHealth {
        SourceWorkerHealth::from_u8(self.status.load(Ordering::Acquire))
    }

    pub(super) fn latch(&self, health: SourceWorkerHealth, failed_mask: u8) {
        self.failed_mask.fetch_or(failed_mask, Ordering::Relaxed);
        if !health.is_terminal() && !health.is_recovering() {
            return;
        }
        loop {
            let current = self.status();
            if current.is_terminal() || (current.is_recovering() && health.is_recovering()) {
                return;
            }
            if current != SourceWorkerHealth::Healthy && !current.is_recovering() {
                return;
            }
            if self
                .status
                .compare_exchange(
                    current as u8,
                    health as u8,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                match health {
                    SourceWorkerHealth::DispatchFailed => {
                        self.dispatch_failures.fetch_add(1, Ordering::Relaxed);
                    }
                    SourceWorkerHealth::CompletionFailed => {
                        self.completion_failures.fetch_add(1, Ordering::Relaxed);
                    }
                    SourceWorkerHealth::DeadlineMiss => {
                        self.deadline_misses.fetch_add(1, Ordering::Relaxed);
                    }
                    SourceWorkerHealth::WorkerExited => {
                        self.worker_exits.fetch_add(1, Ordering::Relaxed);
                    }
                    SourceWorkerHealth::InvalidBlock => {
                        self.invalid_blocks.fetch_add(1, Ordering::Relaxed);
                    }
                    SourceWorkerHealth::Disabled | SourceWorkerHealth::Healthy => {}
                }
                return;
            }
        }
    }

    pub(super) fn recover(&self) -> bool {
        if self
            .status
            .compare_exchange(
                SourceWorkerHealth::DeadlineMiss as u8,
                SourceWorkerHealth::Healthy as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return false;
        }
        self.failed_mask.store(0, Ordering::Relaxed);
        self.deadline_recoveries.fetch_add(1, Ordering::Relaxed);
        true
    }

    pub(super) fn record_completion(&self, sequence: u64) {
        self.last_completion_sequence
            .store(sequence, Ordering::Relaxed);
    }

    pub(super) fn snapshot(&self) -> SourceWorkerHealthSnapshot {
        SourceWorkerHealthSnapshot {
            status: self.status(),
            failed_mask: self.failed_mask.load(Ordering::Relaxed),
            last_completion_sequence: self.last_completion_sequence.load(Ordering::Relaxed),
            dispatch_failures: self.dispatch_failures.load(Ordering::Relaxed),
            completion_failures: self.completion_failures.load(Ordering::Relaxed),
            deadline_misses: self.deadline_misses.load(Ordering::Relaxed),
            deadline_recoveries: self.deadline_recoveries.load(Ordering::Relaxed),
            worker_exits: self.worker_exits.load(Ordering::Relaxed),
            invalid_blocks: self.invalid_blocks.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadline_miss_recovers_and_fatal_status_is_immutable() {
        let health = SourceWorkerHealthState::new(SourceWorkerHealth::Healthy);
        health.latch(SourceWorkerHealth::DeadlineMiss, 0b01);
        health.latch(SourceWorkerHealth::DeadlineMiss, 0b10);
        let missed = health.snapshot();
        assert_eq!(missed.status, SourceWorkerHealth::DeadlineMiss);
        assert_eq!(missed.failed_mask, 0b11);
        assert_eq!(missed.deadline_misses, 1);
        assert_eq!(missed.deadline_recoveries, 0);

        assert!(health.recover());
        let recovered = health.snapshot();
        assert_eq!(recovered.status, SourceWorkerHealth::Healthy);
        assert_eq!(recovered.failed_mask, 0);
        assert_eq!(recovered.deadline_recoveries, 1);
        assert!(!health.recover());

        health.latch(SourceWorkerHealth::CompletionFailed, 0b10);
        health.latch(SourceWorkerHealth::DeadlineMiss, 0b01);
        let fatal = health.snapshot();
        assert_eq!(fatal.status, SourceWorkerHealth::CompletionFailed);
        assert_eq!(fatal.failed_mask, 0b10 | 0b01);
        assert_eq!(fatal.completion_failures, 1);
        assert!(!health.recover());
    }
}
