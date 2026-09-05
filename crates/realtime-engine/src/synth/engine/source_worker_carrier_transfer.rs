use super::bus_chain_owner::BusChainCarrier;
use super::source_worker_lease::OwnerLease;
use super::source_worker_lifecycle::{
    OwnerEnvelope, SourceLanePartitionBundle, SourceWorkerScratch,
};
use super::source_worker_transfer::{
    can_install_source_partition_bundle, install_source_partition_bundle_after_check,
    source_partitions_vacant, take_source_partition_bundles,
};
use super::SynthEngine;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

struct OwnerReturnData {
    generation: u64,
    parity: usize,
    scratch: SourceWorkerScratch,
    partitions: SourceLanePartitionBundle,
    #[cfg(feature = "routing-tree-benchmark")]
    routing_tree: Option<super::routing_tree_worker::RoutingTreeOwnerData>,
}

pub(super) fn with_both_source_owners<R>(
    engine: &mut SynthEngine,
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    operation: impl FnOnce(
        &mut SynthEngine,
        [&SourceWorkerScratch; 2],
        &mut [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
    ) -> R,
) -> Result<R, ()> {
    with_both_source_owners_inner(engine, first, second, true, None, operation)
}

pub(super) fn with_both_source_owners_preserving_carriers<R>(
    engine: &mut SynthEngine,
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    expected_residency: &[u8; super::super::types::BUS_COUNT],
    operation: impl FnOnce(
        &mut SynthEngine,
        [&SourceWorkerScratch; 2],
        &mut [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
    ) -> R,
) -> Result<R, ()> {
    with_both_source_owners_inner(
        engine,
        first,
        second,
        false,
        Some(expected_residency),
        operation,
    )
}

#[cfg(feature = "routing-tree-benchmark")]
pub(super) fn with_both_source_owners_for_routing_tree_controls<R>(
    engine: &mut SynthEngine,
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    operation: impl FnOnce(
        &mut SynthEngine,
        [&SourceWorkerScratch; 2],
        &mut [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
    ) -> Result<R, ()>,
) -> Result<R, ()> {
    with_both_source_owners(engine, first, second, |engine, scratch, carriers| {
        if !sync_routing_tree_spread_states_to_engine(engine, carriers) {
            return Err(());
        }
        let result = operation(engine, scratch, carriers);
        if !sync_routing_tree_spread_states_to_carriers(engine, carriers) {
            return Err(());
        }
        result
    })?
}

fn with_both_source_owners_inner<R>(
    engine: &mut SynthEngine,
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    install_bus_owners: bool,
    expected_residency: Option<&[u8; super::super::types::BUS_COUNT]>,
    operation: impl FnOnce(
        &mut SynthEngine,
        [&SourceWorkerScratch; 2],
        &mut [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
    ) -> R,
) -> Result<R, ()> {
    let Some(first_owner) = first.take_owner() else {
        return Err(());
    };
    let Some(second_owner) = second.take_owner() else {
        first.restore_owner(first_owner);
        return Err(());
    };
    if !valid_owner_pair(
        engine,
        &first_owner,
        &second_owner,
        first_owner.runtime_generation,
        expected_residency,
    ) {
        first.restore_owner(first_owner);
        second.restore_owner(second_owner);
        return Err(());
    }
    let OwnerEnvelope {
        parity: first_parity,
        runtime_generation: first_generation,
        partitions: first_partitions,
        scratch: first_scratch,
        bus_carriers: first_carriers,
        #[cfg(feature = "routing-tree-benchmark")]
            routing_tree: first_routing_tree,
    } = first_owner;
    let OwnerEnvelope {
        parity: second_parity,
        runtime_generation: second_generation,
        partitions: second_partitions,
        scratch: second_scratch,
        bus_carriers: second_carriers,
        #[cfg(feature = "routing-tree-benchmark")]
            routing_tree: second_routing_tree,
    } = second_owner;
    let mut bus_carriers = combine_bus_carriers(first_carriers, second_carriers);
    let operation_result = catch_unwind(AssertUnwindSafe(|| {
        install_source_partition_bundle_after_check(engine, first_parity, first_partitions);
        install_source_partition_bundle_after_check(engine, second_parity, second_partitions);
        if install_bus_owners {
            install_bus_chain_owners_after_check(engine, &mut bus_carriers);
        }
        let result = operation(engine, [&first_scratch, &second_scratch], &mut bus_carriers);
        let partitions = take_source_partition_bundles(engine);
        let carriers = take_bus_chain_owners(engine, &mut bus_carriers);
        (result, partitions, carriers)
    }));
    match operation_result {
        Ok((result, Some((first_partitions, second_partitions)), true)) => {
            restore_owner_pair(
                first,
                second,
                OwnerReturnData {
                    generation: first_generation,
                    parity: first_parity,
                    scratch: first_scratch,
                    partitions: first_partitions,
                    #[cfg(feature = "routing-tree-benchmark")]
                    routing_tree: first_routing_tree,
                },
                OwnerReturnData {
                    generation: second_generation,
                    parity: second_parity,
                    scratch: second_scratch,
                    partitions: second_partitions,
                    #[cfg(feature = "routing-tree-benchmark")]
                    routing_tree: second_routing_tree,
                },
                bus_carriers,
            );
            Ok(result)
        }
        Ok((_, Some((first_partitions, second_partitions)), false)) => {
            let _ = take_bus_chain_owners(engine, &mut bus_carriers);
            restore_owner_pair(
                first,
                second,
                OwnerReturnData {
                    generation: first_generation,
                    parity: first_parity,
                    scratch: first_scratch,
                    partitions: first_partitions,
                    #[cfg(feature = "routing-tree-benchmark")]
                    routing_tree: first_routing_tree,
                },
                OwnerReturnData {
                    generation: second_generation,
                    parity: second_parity,
                    scratch: second_scratch,
                    partitions: second_partitions,
                    #[cfg(feature = "routing-tree-benchmark")]
                    routing_tree: second_routing_tree,
                },
                bus_carriers,
            );
            Err(())
        }
        Ok((_, None, false)) | Ok((_, None, true)) => Err(()),
        Err(payload) => {
            let partitions = take_source_partition_bundles(engine);
            let _ = take_bus_chain_owners(engine, &mut bus_carriers);
            if let Some((first_partitions, second_partitions)) = partitions {
                restore_owner_pair(
                    first,
                    second,
                    OwnerReturnData {
                        generation: first_generation,
                        parity: first_parity,
                        scratch: first_scratch,
                        partitions: first_partitions,
                        #[cfg(feature = "routing-tree-benchmark")]
                        routing_tree: first_routing_tree,
                    },
                    OwnerReturnData {
                        generation: second_generation,
                        parity: second_parity,
                        scratch: second_scratch,
                        partitions: second_partitions,
                        #[cfg(feature = "routing-tree-benchmark")]
                        routing_tree: second_routing_tree,
                    },
                    bus_carriers,
                );
            }
            resume_unwind(payload);
        }
    }
}

fn valid_owner_pair(
    engine: &SynthEngine,
    first: &OwnerEnvelope,
    second: &OwnerEnvelope,
    runtime_generation: u64,
    expected_residency: Option<&[u8; super::super::types::BUS_COUNT]>,
) -> bool {
    first.parity == 0
        && second.parity == 1
        && first.runtime_generation == runtime_generation
        && second.runtime_generation == runtime_generation
        && first.partitions.synth.parity() == first.parity
        && first.partitions.sample.parity() == first.parity
        && second.partitions.synth.parity() == second.parity
        && second.partitions.sample.parity() == second.parity
        && can_install_source_partition_bundle(engine, first.parity)
        && can_install_source_partition_bundle(engine, second.parity)
        && source_partitions_vacant(engine)
        && engine.bus_chains.is_empty()
        && engine.bus_pan_pos.len() <= super::super::types::BUS_COUNT
        && valid_bus_carrier_pair(first, second, engine.bus_pan_pos.len(), expected_residency)
}

fn valid_bus_carrier_pair(
    first: &OwnerEnvelope,
    second: &OwnerEnvelope,
    bus_count: usize,
    expected_residency: Option<&[u8; super::super::types::BUS_COUNT]>,
) -> bool {
    (0..super::super::types::BUS_COUNT).all(|logical_bus_id| {
        if usize::from(first.bus_carriers[logical_bus_id].is_some())
            + usize::from(second.bus_carriers[logical_bus_id].is_some())
            != 1
        {
            return false;
        }
        let carrier = first.bus_carriers[logical_bus_id]
            .as_ref()
            .or_else(|| second.bus_carriers[logical_bus_id].as_ref());
        let Some(carrier) = carrier else {
            return false;
        };
        if carrier.logical_bus_id != logical_bus_id || !carrier.within_worker_capacity() {
            return false;
        }
        let actual_parity = if first.bus_carriers[logical_bus_id].is_some() {
            first.parity
        } else {
            second.parity
        };
        let expected_parity = expected_residency.map_or_else(
            || {
                carrier
                    .owner
                    .as_ref()
                    .and_then(|owner| owner.assigned_worker)
                    .unwrap_or(logical_bus_id % 2) as u8
            },
            |residency| residency[logical_bus_id],
        );
        carrier.owner.is_some() == (logical_bus_id < bus_count)
            && actual_parity == usize::from(expected_parity)
            && carrier
                .owner
                .as_ref()
                .and_then(|owner| owner.assigned_worker)
                .is_none_or(|assigned_worker| assigned_worker == actual_parity)
    })
}

pub(super) fn valid_bus_completion_owner(
    owner: &OwnerEnvelope,
    expected_residency: &[u8; super::super::types::BUS_COUNT],
) -> bool {
    owner
        .bus_carriers
        .iter()
        .enumerate()
        .all(|(logical_bus_id, carrier)| {
            let Some(carrier) = carrier.as_ref() else {
                return true;
            };
            carrier.logical_bus_id == logical_bus_id
                && carrier.within_worker_capacity()
                && expected_residency[logical_bus_id] == owner.parity as u8
                && carrier
                    .owner
                    .as_ref()
                    .and_then(|owner| owner.assigned_worker)
                    .is_none_or(|assigned_worker| assigned_worker == owner.parity)
        })
}

fn combine_bus_carriers(
    mut first: [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
    mut second: [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
) -> [Option<BusChainCarrier>; super::super::types::BUS_COUNT] {
    std::array::from_fn(|logical_bus_id| {
        first[logical_bus_id]
            .take()
            .or_else(|| second[logical_bus_id].take())
    })
}

fn install_bus_chain_owners_after_check(
    engine: &mut SynthEngine,
    carriers: &mut [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
) {
    for carrier in carriers.iter_mut().flatten() {
        if let Some(owner) = carrier.owner.take() {
            engine.bus_chains.push(owner);
        }
    }
}

fn take_bus_chain_owners(
    engine: &mut SynthEngine,
    carriers: &mut [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
) -> bool {
    if engine.bus_chains.len() > super::super::types::BUS_COUNT {
        return false;
    }
    let mut seen = [false; super::super::types::BUS_COUNT];
    if engine.bus_chains.iter().any(|owner| {
        let Some(seen) = seen.get_mut(owner.logical_bus_id) else {
            return true;
        };
        if *seen {
            return true;
        }
        *seen = true;
        false
    }) {
        return false;
    }
    let mut owners = std::mem::take(&mut engine.bus_chains);
    for owner in owners.drain(..) {
        let logical_bus_id = owner.logical_bus_id;
        let carrier = carriers[logical_bus_id]
            .as_mut()
            .expect("validated persistent bus carrier");
        debug_assert!(carrier.owner.is_none());
        carrier.owner = Some(owner);
    }
    engine.bus_chains = owners;
    true
}

fn restore_owner_pair(
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    first_data: OwnerReturnData,
    second_data: OwnerReturnData,
    mut carriers: [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
) {
    let mut first_carriers = std::array::from_fn(|_| None);
    let mut second_carriers = std::array::from_fn(|_| None);
    for (logical_bus_id, carrier) in carriers.iter_mut().enumerate() {
        let Some(carrier) = carrier.take() else {
            continue;
        };
        let parity = carrier
            .owner
            .as_ref()
            .and_then(|owner| owner.assigned_worker)
            .unwrap_or(logical_bus_id % 2);
        if parity == first_data.parity {
            first_carriers[logical_bus_id] = Some(carrier);
        } else {
            second_carriers[logical_bus_id] = Some(carrier);
        }
    }
    first.restore_owner(OwnerEnvelope {
        runtime_generation: first_data.generation,
        parity: first_data.parity,
        partitions: first_data.partitions,
        scratch: first_data.scratch,
        bus_carriers: first_carriers,
        #[cfg(feature = "routing-tree-benchmark")]
        routing_tree: first_data.routing_tree,
    });
    second.restore_owner(OwnerEnvelope {
        runtime_generation: second_data.generation,
        parity: second_data.parity,
        partitions: second_data.partitions,
        scratch: second_data.scratch,
        bus_carriers: second_carriers,
        #[cfg(feature = "routing-tree-benchmark")]
        routing_tree: second_data.routing_tree,
    });
}

pub(super) fn take_bus_carriers(
    engine: &mut SynthEngine,
) -> [Option<BusChainCarrier>; super::super::types::BUS_COUNT] {
    let mut owners = std::mem::take(&mut engine.bus_chains);
    let mut carriers =
        std::array::from_fn(|logical_bus_id| Some(BusChainCarrier::new(logical_bus_id, None)));
    for owner in owners.drain(..) {
        let logical_bus_id = owner.logical_bus_id;
        carriers[logical_bus_id]
            .as_mut()
            .expect("validated persistent bus identity")
            .owner = Some(owner);
    }
    engine.bus_chains = owners;
    carriers
}

pub(super) fn restore_bus_carriers_to_engine(
    engine: &mut SynthEngine,
    carriers: [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
) {
    for carrier in carriers.into_iter().flatten() {
        if let Some(owner) = carrier.owner {
            engine.bus_chains.push(owner);
        }
    }
}

pub(super) fn split_bus_carriers(
    carriers: [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
) -> [[Option<BusChainCarrier>; super::super::types::BUS_COUNT];
       super::super::types::VOICE_PARTITION_COUNT] {
    let mut homes = std::array::from_fn(|_| std::array::from_fn(|_| None));
    for (logical_bus_id, carrier) in carriers.into_iter().enumerate() {
        homes[logical_bus_id % super::super::types::VOICE_PARTITION_COUNT][logical_bus_id] =
            carrier;
    }
    homes
}

#[cfg(feature = "routing-tree-benchmark")]
fn sync_routing_tree_spread_states_to_engine(
    engine: &mut SynthEngine,
    carriers: &mut [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
) -> bool {
    for (bus, carrier) in carriers.iter_mut().enumerate() {
        let Some(carrier) = carrier.as_mut() else {
            return false;
        };
        let Some(state) = carrier.routing_tree_spread_state.as_mut() else {
            return false;
        };
        if let Some(engine_state) = engine.bus_output_spread_state.get_mut(bus) {
            std::mem::swap(state, engine_state);
        } else if carrier.owner.is_some() {
            return false;
        }
    }
    true
}

#[cfg(feature = "routing-tree-benchmark")]
fn sync_routing_tree_spread_states_to_carriers(
    engine: &mut SynthEngine,
    carriers: &mut [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
) -> bool {
    for (bus, carrier) in carriers.iter_mut().enumerate() {
        let Some(carrier) = carrier.as_mut() else {
            return false;
        };
        let Some(state) = carrier.routing_tree_spread_state.as_mut() else {
            return false;
        };
        if let Some(engine_state) = engine.bus_output_spread_state.get_mut(bus) {
            std::mem::swap(state, engine_state);
        } else if carrier.owner.is_some() {
            return false;
        }
    }
    true
}
