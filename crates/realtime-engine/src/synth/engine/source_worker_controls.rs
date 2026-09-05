#[cfg(all(test, feature = "routing-tree-benchmark"))]
use super::super::source_worker_carrier_transfer;
use super::super::source_worker_health::SourceWorkerHealth;
use super::super::source_worker_transfer;
use super::super::SynthEngine;
use super::{SourceWorkerMode, SourceWorkerRuntime};

impl SourceWorkerRuntime {
    pub fn with_controls_ready<R>(
        &mut self,
        engine: &mut SynthEngine,
        apply: impl FnOnce(&mut SynthEngine) -> R,
    ) -> Option<R> {
        if self.mode == SourceWorkerMode::Inline {
            return Some(apply(engine));
        }
        self.reclaim_available(engine);
        if self.health.status() != SourceWorkerHealth::Healthy
            || self.in_flight_mask != 0
            || self.completed_mask != 0
            || !self.home_is_ready()
        {
            return None;
        }
        let mut first = self.lease_home(0)?;
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            self.latch_completion_failure(0b11);
            return None;
        };
        let load = self.load_snapshot();
        match source_worker_transfer::with_both_source_partitions(
            engine,
            &mut first,
            &mut second,
            |engine, _| engine.with_source_worker_load(load, apply),
        ) {
            Ok(result) => {
                first.return_home();
                second.return_home();
                Some(result)
            }
            Err(()) => {
                self.latch_completion_failure(0b11);
                None
            }
        }
    }

    pub fn with_recovered_owners<R>(
        &mut self,
        engine: &mut SynthEngine,
        inspect: impl FnOnce(&SynthEngine) -> R,
    ) -> Option<R> {
        if self.mode == SourceWorkerMode::Inline {
            return Some(inspect(engine));
        }
        self.reclaim_available(engine);
        if !self.home_is_ready() {
            return None;
        }
        let mut first = self.lease_home(0)?;
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            return None;
        };
        match source_worker_transfer::with_both_source_partitions_read_only(
            engine,
            &mut first,
            &mut second,
            inspect,
        ) {
            Ok(result) => {
                first.return_home();
                second.return_home();
                Some(result)
            }
            Err(()) => None,
        }
    }

    #[cfg(all(test, feature = "routing-tree-benchmark"))]
    pub(crate) fn with_recovered_routing_tree_owners<R>(
        &mut self,
        engine: &mut SynthEngine,
        inspect: impl FnOnce(&SynthEngine) -> R,
    ) -> Option<R> {
        self.reclaim_available(engine);
        if !self.home_is_ready() {
            return None;
        }
        let mut first = self.lease_home(0)?;
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            return None;
        };
        let result =
            source_worker_carrier_transfer::with_both_source_owners_for_routing_tree_controls(
                engine,
                &mut first,
                &mut second,
                |engine, _, _| Ok(inspect(engine)),
            );
        match result {
            Ok(result) => {
                first.return_home();
                second.return_home();
                Some(result)
            }
            Err(()) => None,
        }
    }
}
