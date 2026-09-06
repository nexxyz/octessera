use super::bus_chain_owner::BusChainCarrier;
use super::source_worker_lifecycle::OwnerEnvelope;
use super::SynthEngine;

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

pub(super) fn combine_bus_carriers(
    mut first: [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
    mut second: [Option<BusChainCarrier>; super::super::types::BUS_COUNT],
) -> [Option<BusChainCarrier>; super::super::types::BUS_COUNT] {
    std::array::from_fn(|logical_bus_id| {
        first[logical_bus_id]
            .take()
            .or_else(|| second[logical_bus_id].take())
    })
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
