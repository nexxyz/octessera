use super::super::fx_params::{DuckSource, FilterLfoKind, FxBusParams, FxKind};
use super::super::types::{BUS_COUNT, BUS_SLOTS_PER_BUS, INSTRUMENT_SLOT_COUNT};
use super::render_plan::RenderPlanRoute;
use super::routing_tree_plan::{RoutingTreePlan, INVALID_COMPONENT_ID, ROUTING_NODE_COUNT};
use super::SynthEngine;

pub(super) fn valid_render_plan(engine: &SynthEngine, plan: &RoutingTreePlan) -> bool {
    let expected = RoutingTreePlan::from_render_plan(&engine.render_plan);
    if plan.generation != expected.generation
        || !plan.same_structure(&expected)
        || plan.component_count > ROUTING_NODE_COUNT
        || engine.bus_chains.len() > BUS_COUNT
        || engine.render_plan.bus_fx_slots.len() != engine.bus_chains.len()
        || engine.bus_pan_pos.len() != engine.bus_chains.len()
        || engine.bus_pan_gains_cache.len() != engine.bus_chains.len()
        || engine.bus_volume.len() != engine.bus_chains.len()
        || engine.bus_output_spread_state.len() != engine.bus_chains.len()
    {
        return false;
    }
    if !valid_components(plan) {
        return false;
    }
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        let topology = engine.render_plan.instrument_slots[slot];
        if topology.kind != engine.slot_kind[slot] {
            return false;
        }
        match (topology.route, engine.slot_route[slot]) {
            (RenderPlanRoute::Direct, 0) => {}
            (RenderPlanRoute::Bus(bus), route)
                if route == bus + 1 && bus < engine.bus_chains.len() => {}
            _ => return false,
        }
    }
    for bus in 0..engine.bus_chains.len() {
        let owner = &engine.bus_chains[bus];
        if owner.logical_bus_id != bus {
            return false;
        }
        for slot in 0..BUS_SLOTS_PER_BUS {
            let plan_slot = engine.render_plan.bus_fx_slots[bus][slot];
            if !fx_slot_matches(
                plan_slot.kind,
                plan_slot.duck_source,
                owner.slot_params[slot],
            ) {
                return false;
            }
            if let FxBusParams::Duck { source, .. } = owner.slot_params[slot] {
                if !valid_duck_source(source, engine.bus_chains.len()) {
                    return false;
                }
            }
        }
    }
    true
}

fn valid_components(plan: &RoutingTreePlan) -> bool {
    for component in 0..plan.component_count {
        if plan.component_masks[component] == 0 {
            return false;
        }
    }
    if plan.component_masks[plan.component_count..]
        .iter()
        .any(|mask| *mask != 0)
    {
        return false;
    }
    for node in 0..ROUTING_NODE_COUNT {
        let component = if node < INSTRUMENT_SLOT_COUNT {
            plan.slot_component[node]
        } else {
            plan.bus_component[node - INSTRUMENT_SLOT_COUNT]
        };
        let in_mask = plan
            .component_masks
            .iter()
            .any(|mask| *mask & (1_u16 << node) != 0);
        if component == INVALID_COMPONENT_ID {
            if in_mask {
                return false;
            }
        } else if component as usize >= plan.component_count
            || plan.component_masks[component as usize] & (1_u16 << node) == 0
        {
            return false;
        }
    }
    true
}

fn valid_duck_source(source: DuckSource, bus_count: usize) -> bool {
    match source {
        DuckSource::Instrument(index) => index < INSTRUMENT_SLOT_COUNT,
        DuckSource::Bus(index) => index < bus_count,
    }
}

fn fx_slot_matches(kind: FxKind, duck_source: Option<DuckSource>, params: FxBusParams) -> bool {
    match (kind, duck_source, params) {
        (FxKind::None, None, FxBusParams::None)
        | (FxKind::Tremolo, None, FxBusParams::Tremolo { .. })
        | (FxKind::Delay, None, FxBusParams::Delay { .. })
        | (FxKind::Reverb, None, FxBusParams::Reverb { .. })
        | (FxKind::Glitch, None, FxBusParams::Glitch { .. })
        | (FxKind::AutoPan, None, FxBusParams::AutoPan { .. })
        | (FxKind::Saturator, None, FxBusParams::Saturator { .. })
        | (FxKind::Distortion, None, FxBusParams::Distortion { .. })
        | (FxKind::Bitcrusher, None, FxBusParams::Bitcrusher { .. })
        | (FxKind::Compressor, None, FxBusParams::Compressor { .. })
        | (FxKind::Eq, None, FxBusParams::Eq { .. })
        | (FxKind::Vinyl, None, FxBusParams::Vinyl { .. }) => true,
        (
            FxKind::Vibrato | FxKind::Chorus | FxKind::Flanger,
            None,
            FxBusParams::ModDelay { .. },
        ) => true,
        (
            FxKind::FilterLfo,
            None,
            FxBusParams::FilterLfo {
                kind: FilterLfoKind::FilterLfo,
                ..
            },
        ) => true,
        (
            FxKind::Wah,
            None,
            FxBusParams::FilterLfo {
                kind: FilterLfoKind::Wah,
                ..
            },
        ) => true,
        (FxKind::Duck, Some(expected), FxBusParams::Duck { source, .. }) => expected == source,
        _ => false,
    }
}
