use super::super::synth_voice_pool::SynthVoicePool;
use super::super::types::BUS_COUNT;
#[cfg(feature = "routing-tree-benchmark")]
use super::bus_chain_owner::BusChainOwner;
use super::render_plan::RenderPlan;
use super::routing_tree_plan::{RoutingTreePlan, INVALID_COMPONENT_ID, ROUTING_NODE_COUNT};
use super::sample_voice_pool::SampleVoicePool;
use super::source_worker_load::{
    SOURCE_WORKER_MAX_COST_UNITS, SOURCE_WORKER_SAMPLE_COST_UNITS, SOURCE_WORKER_SYNTH_COST_UNITS,
};
use super::support::InstrumentKind;
#[cfg(feature = "routing-tree-benchmark")]
use super::SynthEngine;
use crate::synth::types::INSTRUMENT_SLOT_COUNT;
#[cfg(feature = "routing-tree-benchmark")]
use crate::synth::types::{
    SAMPLE_VOICE_PARTITION_LANE_CAPACITY, SYNTH_VOICE_PARTITION_LANE_CAPACITY,
};

const WORKER_COUNT: usize = 2;
const INVALID_WORKER: u8 = u8::MAX;

#[cfg(test)]
#[path = "routing_tree_executor_reference.rs"]
mod reference;

#[cfg(feature = "routing-tree-benchmark")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RoutingTreeAssignment {
    pub(super) plan: RoutingTreePlan,
    component_worker: [u8; ROUTING_NODE_COUNT],
}

#[cfg(feature = "routing-tree-benchmark")]
impl RoutingTreeAssignment {
    pub(super) fn worker_for_slot(&self, slot: usize) -> Option<usize> {
        self.plan
            .slot_component
            .get(slot)
            .copied()
            .and_then(|component| self.worker_for_component(component))
    }

    pub(super) fn worker_for_bus(&self, bus: usize) -> Option<usize> {
        self.plan
            .bus_component
            .get(bus)
            .copied()
            .and_then(|component| self.worker_for_component(component))
    }

    fn worker_for_component(&self, component: u8) -> Option<usize> {
        if component == INVALID_COMPONENT_ID {
            return None;
        }
        let component = usize::from(component);
        if component >= self.plan.component_count {
            return None;
        }
        match self.component_worker.get(component).copied()? {
            worker if usize::from(worker) < WORKER_COUNT => Some(usize::from(worker)),
            _ => None,
        }
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn has_same_component_worker_mapping(&self, other: &Self) -> bool {
        self.component_worker == other.component_worker
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn preserve_component_worker_mapping(&mut self, other: &Self) {
        self.component_worker = other.component_worker;
    }

    pub(super) fn validate_engine(&self, engine: &SynthEngine) -> bool {
        if self.plan.component_count > ROUTING_NODE_COUNT
            || self.plan.slot_component.iter().any(|component| {
                *component != INVALID_COMPONENT_ID
                    && usize::from(*component) >= self.plan.component_count
            })
            || self.plan.bus_component.iter().any(|component| {
                *component != INVALID_COMPONENT_ID
                    && usize::from(*component) >= self.plan.component_count
            })
        {
            return false;
        }
        let Some(synth_counts) = active_counts_by_slot(&engine.synth_voice_pool) else {
            return false;
        };
        let Some(sample_counts) = active_sample_counts_by_slot(&engine.sample_voice_pool) else {
            return false;
        };
        let mut component_synth = [0_usize; ROUTING_NODE_COUNT];
        let mut component_sample = [0_usize; ROUTING_NODE_COUNT];
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let component = self.plan.slot_component[slot];
            if component == INVALID_COMPONENT_ID {
                continue;
            }
            if usize::from(component) >= self.plan.component_count {
                return false;
            }
            match engine.slot_kind[slot] {
                InstrumentKind::Synth => {
                    component_synth[component as usize] =
                        component_synth[component as usize].saturating_add(synth_counts[slot]);
                }
                InstrumentKind::Sample => {
                    component_sample[component as usize] =
                        component_sample[component as usize].saturating_add(sample_counts[slot]);
                }
                InstrumentKind::Midi | InstrumentKind::None => {}
            }
        }
        (0..self.plan.component_count).all(|component| {
            component_synth[component] <= SYNTH_VOICE_PARTITION_LANE_CAPACITY
                && component_sample[component] <= SAMPLE_VOICE_PARTITION_LANE_CAPACITY
        }) && validate_engine_partition_residency(engine, self)
    }
}

pub(super) struct RoutingTreeBlockScratch {
    #[cfg(test)]
    worker_left: [Vec<f32>; WORKER_COUNT],
    #[cfg(test)]
    worker_right: [Vec<f32>; WORKER_COUNT],
    #[cfg(test)]
    bus_input: [Vec<f32>; BUS_COUNT],
    component_worker: [u8; ROUTING_NODE_COUNT],
    plan: RoutingTreePlan,
}

impl RoutingTreeBlockScratch {
    pub(super) fn new() -> Self {
        Self {
            #[cfg(test)]
            worker_left: std::array::from_fn(|_| vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES]),
            #[cfg(test)]
            worker_right: std::array::from_fn(|_| vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES]),
            #[cfg(test)]
            bus_input: std::array::from_fn(|_| vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES]),
            component_worker: [INVALID_WORKER; ROUTING_NODE_COUNT],
            plan: RoutingTreePlan::from_render_plan(&RenderPlan::new()),
        }
    }

    #[cfg(test)]
    fn prepare(&mut self, frames: usize) -> bool {
        if frames > super::BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        for worker in 0..WORKER_COUNT {
            self.worker_left[worker][..frames].fill(0.0);
            self.worker_right[worker][..frames].fill(0.0);
        }
        for bus in 0..BUS_COUNT {
            self.bus_input[bus][..frames].fill(0.0);
        }
        self.component_worker.fill(INVALID_WORKER);
        true
    }

    fn assign_workers(
        &mut self,
        plan: RoutingTreePlan,
        instrument_kinds: [InstrumentKind; INSTRUMENT_SLOT_COUNT],
        synth_counts: [usize; INSTRUMENT_SLOT_COUNT],
        sample_counts: [usize; INSTRUMENT_SLOT_COUNT],
        bus_costs: [u16; BUS_COUNT],
        bus_count: usize,
    ) -> bool {
        if bus_count > BUS_COUNT || plan.component_count > ROUTING_NODE_COUNT {
            return false;
        }
        let mut component_cost = [0_u16; ROUTING_NODE_COUNT];
        let mut component_synth = [0_usize; ROUTING_NODE_COUNT];
        let mut component_sample = [0_usize; ROUTING_NODE_COUNT];
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let component = plan.slot_component[slot];
            if component == INVALID_COMPONENT_ID {
                continue;
            }
            if usize::from(component) >= plan.component_count {
                return false;
            }
            let kind = instrument_kinds[slot];
            let count = match kind {
                InstrumentKind::Synth => synth_counts[slot],
                InstrumentKind::Sample => sample_counts[slot],
                InstrumentKind::Midi | InstrumentKind::None => 0,
            };
            let unit = match kind {
                InstrumentKind::Synth => SOURCE_WORKER_SYNTH_COST_UNITS,
                InstrumentKind::Sample => SOURCE_WORKER_SAMPLE_COST_UNITS,
                InstrumentKind::Midi | InstrumentKind::None => 0,
            };
            match kind {
                InstrumentKind::Synth => {
                    component_synth[component as usize] =
                        component_synth[component as usize].saturating_add(count)
                }
                InstrumentKind::Sample => {
                    component_sample[component as usize] =
                        component_sample[component as usize].saturating_add(count)
                }
                InstrumentKind::Midi | InstrumentKind::None => {}
            }
            let Some(cost) = cost_units(count, unit) else {
                return false;
            };
            let Some(total) = component_cost[component as usize].checked_add(cost) else {
                return false;
            };
            component_cost[component as usize] = total;
        }
        for (bus, cost) in bus_costs.iter().copied().take(bus_count).enumerate() {
            let component = plan.bus_component[bus];
            if component == INVALID_COMPONENT_ID || usize::from(component) >= plan.component_count {
                return false;
            }
            let Some(total) = component_cost[component as usize].checked_add(cost) else {
                return false;
            };
            component_cost[component as usize] = total;
        }
        let mut projected = [0_u16; WORKER_COUNT];
        for (component, cost) in component_cost
            .iter()
            .copied()
            .take(plan.component_count)
            .enumerate()
        {
            let worker = usize::from(projected[1] < projected[0]);
            self.component_worker[component] = worker as u8;
            let Some(total) = projected[worker].checked_add(cost) else {
                return false;
            };
            projected[worker] = total;
            if projected[worker] > SOURCE_WORKER_MAX_COST_UNITS {
                return false;
            }
        }
        self.plan = plan;
        true
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn assignment(&self) -> RoutingTreeAssignment {
        RoutingTreeAssignment {
            plan: self.plan,
            component_worker: self.component_worker,
        }
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn assignment_for_engine(engine: &SynthEngine) -> Option<RoutingTreeAssignment> {
        let assignment = Self::assignment_for_engine_unvalidated(engine)?;
        validate_engine_partition_residency(engine, &assignment).then_some(assignment)
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn assignment_for_engine_unvalidated(
        engine: &SynthEngine,
    ) -> Option<RoutingTreeAssignment> {
        let plan = RoutingTreePlan::from_render_plan(&engine.render_plan);
        let synth_counts = active_counts_by_slot(&engine.synth_voice_pool)?;
        let sample_counts = active_sample_counts_by_slot(&engine.sample_voice_pool)?;
        let mut scratch = Self::new();
        if !scratch.assign_workers(
            plan,
            std::array::from_fn(|slot| engine.slot_kind[slot]),
            synth_counts,
            sample_counts,
            std::array::from_fn(|bus| {
                engine
                    .bus_chains
                    .get(bus)
                    .map(BusChainOwner::cost_units)
                    .unwrap_or(0)
            }),
            engine.bus_chains.len(),
        ) {
            return None;
        }
        Some(scratch.assignment())
    }

    #[cfg(test)]
    pub(super) fn worker_for_slot(&self, slot: usize) -> Option<usize> {
        self.plan
            .slot_component
            .get(slot)
            .copied()
            .and_then(|component| self.worker_for_component(component))
    }

    #[cfg(test)]
    pub(super) fn worker_for_bus(&self, bus: usize) -> Option<usize> {
        self.plan
            .bus_component
            .get(bus)
            .copied()
            .and_then(|component| self.worker_for_component(component))
    }

    #[cfg(test)]
    fn worker_for_component(&self, component: u8) -> Option<usize> {
        if component == INVALID_COMPONENT_ID {
            return None;
        }
        let component = usize::from(component);
        if component >= self.plan.component_count {
            return None;
        }
        match self.component_worker.get(component).copied()? {
            worker if usize::from(worker) < WORKER_COUNT => Some(usize::from(worker)),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn assignment_for_test(&self) -> ([u8; ROUTING_NODE_COUNT], RoutingTreePlan) {
        (self.component_worker, self.plan)
    }

    #[cfg(test)]
    pub(super) fn assign_workers_for_test(
        &mut self,
        plan: RoutingTreePlan,
        instrument_kinds: [InstrumentKind; INSTRUMENT_SLOT_COUNT],
        synth_counts: [usize; INSTRUMENT_SLOT_COUNT],
        sample_counts: [usize; INSTRUMENT_SLOT_COUNT],
        bus_costs: [u16; BUS_COUNT],
        bus_count: usize,
    ) -> bool {
        self.assign_workers(
            plan,
            instrument_kinds,
            synth_counts,
            sample_counts,
            bus_costs,
            bus_count,
        )
    }

    #[cfg(test)]
    pub(super) fn set_assignment_for_test(
        &mut self,
        component_worker: [u8; ROUTING_NODE_COUNT],
        plan: RoutingTreePlan,
    ) {
        self.component_worker = component_worker;
        self.plan = plan;
    }

    #[cfg(test)]
    pub(super) fn worker_outputs_for_test(&self, frame: usize) -> [(f32, f32); WORKER_COUNT] {
        std::array::from_fn(|worker| {
            (
                self.worker_left[worker][frame],
                self.worker_right[worker][frame],
            )
        })
    }
}

fn cost_units(count: usize, unit: u16) -> Option<u16> {
    count
        .checked_mul(usize::from(unit))
        .and_then(|cost| u16::try_from(cost).ok())
}

fn active_counts_by_slot(pool: &SynthVoicePool) -> Option<[usize; INSTRUMENT_SLOT_COUNT]> {
    let mut counts = [0; INSTRUMENT_SLOT_COUNT];
    for (slot, count) in counts.iter_mut().enumerate() {
        *count = pool.active_count_for_slot(slot)?;
    }
    Some(counts)
}

fn active_sample_counts_by_slot(pool: &SampleVoicePool) -> Option<[usize; INSTRUMENT_SLOT_COUNT]> {
    let mut counts = [0; INSTRUMENT_SLOT_COUNT];
    for (slot, count) in counts.iter_mut().enumerate() {
        *count = pool.active_count_for_slot(slot)?;
    }
    Some(counts)
}

#[cfg(feature = "routing-tree-benchmark")]
fn validate_engine_partition_residency(
    engine: &SynthEngine,
    assignment: &RoutingTreeAssignment,
) -> bool {
    for lane in 0..super::super::types::SYNTH_VOICE_LANE_CAPACITY {
        let Some(voice) = engine.synth_voice_pool.lane(lane) else {
            return false;
        };
        if voice.active
            && assignment.worker_for_slot(voice.instrument_slot as usize)
                != Some(lane % WORKER_COUNT)
        {
            return false;
        }
    }
    for lane in 0..super::super::types::SAMPLE_VOICE_LANE_CAPACITY {
        let Some(voice) = engine.sample_voice_pool.lane(lane) else {
            return false;
        };
        if voice.active
            && assignment.worker_for_slot(voice.instrument_slot as usize)
                != Some(lane % WORKER_COUNT)
        {
            return false;
        }
    }
    true
}
