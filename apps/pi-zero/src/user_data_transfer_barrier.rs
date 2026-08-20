use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

pub(crate) type RestorePreflight = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;

#[derive(Clone)]
pub(crate) struct StoreWriteBarrier {
    generation: Arc<AtomicU64>,
    blocked: Arc<AtomicBool>,
}

impl StoreWriteBarrier {
    pub(crate) fn new() -> Self {
        Self {
            generation: Arc::new(AtomicU64::new(0)),
            blocked: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(crate) fn is_blocked(&self) -> bool {
        self.blocked.load(Ordering::Acquire)
    }

    pub(crate) fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.blocked.store(true, Ordering::Release);
    }

    pub(crate) fn finish(&self, restored: bool) {
        self.blocked.store(restored, Ordering::Release);
    }

    pub(crate) fn acknowledge(&self) {
        self.blocked.store(false, Ordering::Release);
    }
}
