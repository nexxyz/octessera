use super::super::SynthEngine;
use super::{SourceWorkerLoadSnapshot, SourceWorkerRuntime};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::atomic::Ordering;

impl SynthEngine {
    pub(super) fn with_source_worker_load<R>(
        &mut self,
        load: Option<SourceWorkerLoadSnapshot>,
        apply: impl FnOnce(&mut SynthEngine) -> R,
    ) -> R {
        let previous = self.source_worker_load;
        self.source_worker_load = load;
        let result = catch_unwind(AssertUnwindSafe(|| apply(self)));
        self.source_worker_load = previous;
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }
}

impl Drop for SourceWorkerRuntime {
    fn drop(&mut self) {
        if let Some(close) = self.runtime_close.as_ref() {
            close.closed.store(true, Ordering::Release);
        }
    }
}
