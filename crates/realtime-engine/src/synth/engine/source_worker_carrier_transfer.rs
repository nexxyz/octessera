use super::bus_chain_owner::BusChainCarrier;
use super::source_worker_carrier_transfer_bus::combine_bus_carriers;
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

pub(super) use super::source_worker_carrier_transfer_bus::{
    restore_bus_carriers_to_engine, split_bus_carriers, take_bus_carriers,
    valid_bus_completion_owner,
};

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
    with_both_source_owners_inner(engine, first, second, true, None, false, operation)
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
        false,
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
    with_both_source_owners_inner(
        engine,
        first,
        second,
        true,
        None,
        true,
        |engine, scratch, carriers| {
            if !super::routing_tree_state::sync_routing_tree_spread_states_to_engine(
                engine, carriers,
            ) {
                return Err(());
            }
            let result = operation(engine, scratch, carriers);
            if !super::routing_tree_state::sync_routing_tree_spread_states_to_carriers(
                engine, carriers,
            ) {
                return Err(());
            }
            result
        },
    )?
}

fn with_both_source_owners_inner<R>(
    engine: &mut SynthEngine,
    first: &mut OwnerLease,
    second: &mut OwnerLease,
    install_bus_owners: bool,
    expected_residency: Option<&[u8; super::super::types::BUS_COUNT]>,
    routing_tree_controls: bool,
    operation: impl FnOnce(
        &mut SynthEngine,
        [&SourceWorkerScratch; 2],
        &mut [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
    ) -> R,
) -> Result<R, ()> {
    #[cfg(not(feature = "routing-tree-benchmark"))]
    let _ = routing_tree_controls;
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
    #[cfg(feature = "routing-tree-benchmark")]
    if routing_tree_controls {
        let Some(assignment) = engine.routing_tree_assignment() else {
            first.restore_owner(first_owner);
            second.restore_owner(second_owner);
            return Err(());
        };
        let Some(first_routing_tree) = first_owner.routing_tree.as_ref() else {
            first.restore_owner(first_owner);
            second.restore_owner(second_owner);
            return Err(());
        };
        let Some(second_routing_tree) = second_owner.routing_tree.as_ref() else {
            first.restore_owner(first_owner);
            second.restore_owner(second_owner);
            return Err(());
        };
        if !super::routing_tree_state::preflight_owner_state(
            engine,
            &assignment,
            first_routing_tree,
            second_routing_tree,
        ) {
            first.restore_owner(first_owner);
            second.restore_owner(second_owner);
            return Err(());
        }
    }
    let OwnerEnvelope {
        parity: first_parity,
        runtime_generation: first_generation,
        partitions: first_partitions,
        scratch: first_scratch,
        bus_carriers: first_carriers,
        #[cfg(feature = "routing-tree-benchmark")]
            routing_tree: mut first_routing_tree,
    } = first_owner;
    let OwnerEnvelope {
        parity: second_parity,
        runtime_generation: second_generation,
        partitions: second_partitions,
        scratch: second_scratch,
        bus_carriers: second_carriers,
        #[cfg(feature = "routing-tree-benchmark")]
            routing_tree: mut second_routing_tree,
    } = second_owner;
    let mut bus_carriers = combine_bus_carriers(first_carriers, second_carriers);
    let operation_result = catch_unwind(AssertUnwindSafe(|| {
        install_source_partition_bundle_after_check(engine, first_parity, first_partitions);
        install_source_partition_bundle_after_check(engine, second_parity, second_partitions);
        if install_bus_owners {
            install_bus_chain_owners_after_check(engine, &mut bus_carriers);
        }
        #[cfg(feature = "routing-tree-benchmark")]
        if routing_tree_controls {
            let Some(first_routing_tree) = first_routing_tree.as_mut() else {
                panic!("routing-tree owner state missing");
            };
            let Some(second_routing_tree) = second_routing_tree.as_mut() else {
                panic!("routing-tree owner state missing");
            };
            if !super::routing_tree_state::move_owner_state_to_engine(
                engine,
                first_routing_tree,
                second_routing_tree,
            ) {
                panic!("routing-tree owner state transfer failed");
            }
        }
        let result = operation(engine, [&first_scratch, &second_scratch], &mut bus_carriers);
        #[cfg(feature = "routing-tree-benchmark")]
        if routing_tree_controls {
            let Some(assignment) = engine.routing_tree_assignment() else {
                panic!("routing-tree assignment missing after control");
            };
            let Some(first_routing_tree) = first_routing_tree.as_mut() else {
                panic!("routing-tree owner state missing");
            };
            let Some(second_routing_tree) = second_routing_tree.as_mut() else {
                panic!("routing-tree owner state missing");
            };
            if !super::routing_tree_state::move_engine_state_to_owners(
                engine,
                &assignment,
                first_routing_tree,
                second_routing_tree,
            ) {
                panic!("routing-tree owner state transfer failed");
            }
        }
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
