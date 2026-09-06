use super::super::routing_tree_worker::RoutingTreeWorkerContext;
use super::super::source_worker_carrier_transfer;
use super::super::source_worker_health::SourceWorkerHealth;
use super::super::source_worker_protocol::SourceWorkerMode;
use super::super::SynthEngine;
use super::SourceWorkerRuntime;

impl SourceWorkerRuntime {
    pub(in crate::synth::engine) fn with_routing_tree_controls_ready<R>(
        &mut self,
        engine: &mut SynthEngine,
        effective_sample_clock: u64,
        apply: impl FnOnce(&mut SynthEngine) -> Result<R, ()>,
    ) -> Option<R> {
        if self.mode != SourceWorkerMode::RoutingTreePersistent
            || self.in_flight_mask != 0
            || self.completed_mask != 0
            || !self.home_is_ready()
            || self.health.status() != SourceWorkerHealth::Healthy
        {
            return None;
        }
        let mut first = self.lease_home(0)?;
        let Some(mut second) = self.lease_home(1) else {
            first.return_fault();
            return None;
        };
        let Some(assignment) = engine.routing_tree_assignment() else {
            first.return_fault();
            second.return_fault();
            return None;
        };
        let Some(context) = RoutingTreeWorkerContext::from_engine(engine, &assignment) else {
            first.return_fault();
            second.return_fault();
            return None;
        };
        super::routing_tree_pipeline::reassert_routing_tree_bus_assignments(
            &mut first,
            &mut second,
            &context,
        );
        let load = self.load_snapshot();
        let result =
            source_worker_carrier_transfer::with_both_source_owners_for_routing_tree_controls(
                engine,
                &mut first,
                &mut second,
                |engine, _, _| {
                    engine.with_source_worker_load(load, |engine| {
                        engine.with_routing_tree_source_event_sample_clock(
                            effective_sample_clock,
                            apply,
                        )
                    })
                },
            );
        match result {
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
}
