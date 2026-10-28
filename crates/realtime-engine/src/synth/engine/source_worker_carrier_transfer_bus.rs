use super::bus_chain_owner::BusChainCarrier;
use super::SynthEngine;

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
            .expect("validated bus identity")
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
