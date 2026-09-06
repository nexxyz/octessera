#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
use std::sync::atomic::AtomicBool;
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::Ordering;
#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
use std::sync::{Arc, Weak};

#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
pub(super) struct SourceWorkerCloseState {
    pub(super) closed: AtomicBool,
    pub(super) generation: u64,
}

pub struct SourceWorkerRetirement {
    #[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
    pub(super) close: Weak<SourceWorkerCloseState>,
    #[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
    pub(super) generation: u64,
}

#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
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
