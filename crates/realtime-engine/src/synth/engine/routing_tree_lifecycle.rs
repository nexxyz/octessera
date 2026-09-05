use super::bus_chain_owner::BusChainCarrier;
use super::routing_tree_executor::RoutingTreeAssignment;
use super::routing_tree_worker::RoutingTreeOwnerData;
use super::source_worker_carrier_transfer;
use super::source_worker_lifecycle::{
    OwnerEnvelope, SourceLanePartitionBundle, SourceWorkerLifecycle, SourceWorkerScratch,
};
use super::source_worker_protocol::{SourceWorkerSetupError, SourceWorkerStartHook};
use super::source_worker_transfer;
use super::{SourceWorkerRuntime, SynthEngine};
use crate::synth::types::BUS_COUNT;

pub(super) fn start_prewarmed(
    engine: &mut SynthEngine,
    active_frames: usize,
) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
    start_prewarmed_with_hook(engine, active_frames, None)
}

pub(super) fn start_prewarmed_with_hook(
    engine: &mut SynthEngine,
    active_frames: usize,
    start_hook: Option<SourceWorkerStartHook>,
) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
    if !engine.enable_routing_tree() {
        return Err(SourceWorkerSetupError::RoutingTreeAdmissionUnavailable);
    }
    let mut lifecycle =
        SourceWorkerLifecycle::start_routing_tree_with_hold_and_hook(false, start_hook)?;
    if let Err(error) = lifecycle.prewarm() {
        lifecycle.mark_runtime_closed();
        let _ = lifecycle.shutdown_after_runtime_drop();
        return Err(error);
    }
    let bus_carriers = source_worker_carrier_transfer::take_bus_carriers(engine);
    let Some(home_partitions) = source_worker_transfer::take_source_partition_bundles(engine)
    else {
        source_worker_carrier_transfer::restore_bus_carriers_to_engine(engine, bus_carriers);
        lifecycle.mark_runtime_closed();
        return Err(SourceWorkerSetupError::PartitionsUnavailable);
    };
    let Some((synth_scratch, sample_scratch)) = engine.take_inline_source_scratch() else {
        restore_partitions(engine, home_partitions);
        source_worker_carrier_transfer::restore_bus_carriers_to_engine(engine, bus_carriers);
        lifecycle.mark_runtime_closed();
        return Err(SourceWorkerSetupError::InlineSourceExecutorUnavailable);
    };
    let Some([first_scratch, second_scratch]) =
        SourceWorkerScratch::from_inline_scratch(synth_scratch, sample_scratch)
    else {
        restore_partitions(engine, home_partitions);
        source_worker_carrier_transfer::restore_bus_carriers_to_engine(engine, bus_carriers);
        lifecycle.mark_runtime_closed();
        return Err(SourceWorkerSetupError::InlineSourceExecutorUnavailable);
    };
    let mut bus_carriers = bus_carriers;
    for carrier in bus_carriers.iter_mut().flatten() {
        carrier.routing_tree_spread_state = Some(
            super::render_routing::FxBusOutputSpreadState::new(engine.sample_rate),
        );
    }
    let Some(assignment) = engine.routing_tree_assignment() else {
        restore_partitions(engine, home_partitions);
        source_worker_carrier_transfer::restore_bus_carriers_to_engine(engine, bus_carriers);
        lifecycle.mark_runtime_closed();
        return Err(SourceWorkerSetupError::RoutingTreeAdmissionUnavailable);
    };
    let [first_carriers, second_carriers] = split_initial_carriers(bus_carriers, &assignment);
    if !lifecycle.seed_home([
        OwnerEnvelope {
            runtime_generation: lifecycle.runtime_generation(),
            parity: 0,
            partitions: home_partitions.0,
            scratch: first_scratch,
            bus_carriers: first_carriers,
            routing_tree: Some(RoutingTreeOwnerData::new()),
        },
        OwnerEnvelope {
            runtime_generation: lifecycle.runtime_generation(),
            parity: 1,
            partitions: home_partitions.1,
            scratch: second_scratch,
            bus_carriers: second_carriers,
            routing_tree: Some(RoutingTreeOwnerData::new()),
        },
    ]) {
        lifecycle.mark_runtime_closed();
        return Err(SourceWorkerSetupError::WorkerChannelsUnavailable);
    }
    let Some(mut runtime) =
        SourceWorkerRuntime::new_routing_tree(&lifecycle, engine.sample_rate, active_frames)
    else {
        lifecycle.mark_runtime_closed();
        return Err(SourceWorkerSetupError::WorkerChannelsUnavailable);
    };
    if !runtime.prime_routing_tree(engine, active_frames) {
        let _ = runtime.retire();
        lifecycle.mark_runtime_closed();
        let _ = lifecycle.shutdown_after_runtime_drop();
        return Err(SourceWorkerSetupError::RoutingTreeAdmissionUnavailable);
    }
    Ok((lifecycle, runtime))
}

impl SourceWorkerLifecycle {
    pub fn start_routing_tree_prewarmed(
        engine: &mut SynthEngine,
        active_frames: usize,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        start_prewarmed(engine, active_frames)
    }

    pub fn start_routing_tree_prewarmed_with_hook(
        engine: &mut SynthEngine,
        active_frames: usize,
        start_hook: SourceWorkerStartHook,
    ) -> Result<(SourceWorkerLifecycle, SourceWorkerRuntime), SourceWorkerSetupError> {
        start_prewarmed_with_hook(engine, active_frames, Some(start_hook))
    }
}

fn split_initial_carriers(
    carriers: [Option<BusChainCarrier>; BUS_COUNT],
    assignment: &RoutingTreeAssignment,
) -> [[Option<BusChainCarrier>; BUS_COUNT]; 2] {
    let mut result = std::array::from_fn(|_| std::array::from_fn(|_| None));
    for (bus, carrier) in carriers.into_iter().enumerate() {
        let worker = assignment.worker_for_bus(bus).unwrap_or(bus % 2);
        result[worker][bus] = carrier;
    }
    result
}

fn restore_partitions(
    engine: &mut SynthEngine,
    partitions: (SourceLanePartitionBundle, SourceLanePartitionBundle),
) {
    source_worker_transfer::restore_source_partition_bundle(engine, partitions.0);
    source_worker_transfer::restore_source_partition_bundle(engine, partitions.1);
}
