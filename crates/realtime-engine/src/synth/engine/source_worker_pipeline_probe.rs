use std::sync::atomic::{AtomicU64, Ordering};

pub struct RoutingTreePipelineProbe {
    last_dispatch: AtomicU64,
    last_dispatch_base_sample_clock: AtomicU64,
    last_coordinator: AtomicU64,
    last_coordinator_base_sample_clock: AtomicU64,
    ordering_violations: AtomicU64,
}

impl RoutingTreePipelineProbe {
    pub fn new() -> Self {
        Self {
            last_dispatch: AtomicU64::new(u64::MAX),
            last_dispatch_base_sample_clock: AtomicU64::new(u64::MAX),
            last_coordinator: AtomicU64::new(u64::MAX),
            last_coordinator_base_sample_clock: AtomicU64::new(u64::MAX),
            ordering_violations: AtomicU64::new(0),
        }
    }

    pub(super) fn record_dispatch(&self, sequence: u64, base_sample_clock: u64) {
        self.last_dispatch.store(sequence, Ordering::Release);
        self.last_dispatch_base_sample_clock
            .store(base_sample_clock, Ordering::Release);
    }

    pub(super) fn record_coordinator(&self, sequence: u64, base_sample_clock: u64) {
        let dispatch = self.last_dispatch.load(Ordering::Acquire);
        if dispatch <= sequence {
            self.ordering_violations.fetch_add(1, Ordering::Relaxed);
        }
        self.last_coordinator.store(sequence, Ordering::Release);
        self.last_coordinator_base_sample_clock
            .store(base_sample_clock, Ordering::Release);
    }

    pub fn ordering_violations(&self) -> u64 {
        self.ordering_violations.load(Ordering::Acquire)
    }

    pub fn last_dispatch(&self) -> u64 {
        self.last_dispatch.load(Ordering::Acquire)
    }

    pub fn last_dispatch_base_sample_clock(&self) -> u64 {
        self.last_dispatch_base_sample_clock.load(Ordering::Acquire)
    }

    pub fn last_coordinator(&self) -> u64 {
        self.last_coordinator.load(Ordering::Acquire)
    }

    pub fn last_coordinator_base_sample_clock(&self) -> u64 {
        self.last_coordinator_base_sample_clock
            .load(Ordering::Acquire)
    }
}

impl Default for RoutingTreePipelineProbe {
    fn default() -> Self {
        Self::new()
    }
}
