use super::source_worker_lifecycle::SourceWorkerCloseState;
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

pub struct SourceWorkerRetirement {
    pub(super) close: Weak<SourceWorkerCloseState>,
    pub(super) generation: u64,
}

impl SourceWorkerRetirement {
    pub(super) fn new(close: &Arc<SourceWorkerCloseState>) -> Self {
        Self {
            generation: close.generation,
            close: Arc::downgrade(close),
        }
    }

    pub(super) fn inline() -> Self {
        Self {
            close: Weak::new(),
            generation: 0,
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub struct SourceWorkerHoldControl {
    holds: [Arc<AtomicBool>; 2],
}

#[cfg(any(test, feature = "test-support"))]
impl SourceWorkerHoldControl {
    pub(super) fn new(holds: [Arc<AtomicBool>; 2]) -> Self {
        Self { holds }
    }

    pub fn release(&self) {
        for hold in &self.holds {
            hold.store(false, Ordering::Release);
        }
    }
}
