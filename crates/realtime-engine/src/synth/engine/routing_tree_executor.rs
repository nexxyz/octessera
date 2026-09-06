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
use crate::synth::types::{SAMPLE_VOICE_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY};

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
    slot_worker: [u8; INSTRUMENT_SLOT_COUNT],
}

#[cfg(feature = "routing-tree-benchmark")]
impl RoutingTreeAssignment {
    pub(super) fn worker_for_slot(&self, slot: usize) -> Option<usize> {
        let component = self.plan.slot_component.get(slot).copied()?;
        if component != INVALID_COMPONENT_ID {
            return self.worker_for_component(component);
        }
        match self.slot_worker.get(slot).copied()? {
            worker if usize::from(worker) < WORKER_COUNT => Some(usize::from(worker)),
            _ => None,
        }
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
        self.component_worker == other.component_worker && self.slot_worker == other.slot_worker
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn preserve_component_worker_mapping(&mut self, other: &Self) {
        let mut preserved = [INVALID_WORKER; ROUTING_NODE_COUNT];
        for (next_component, preserved_worker) in preserved
            .iter_mut()
            .enumerate()
            .take(self.plan.component_count)
        {
            let next_mask = self.plan.component_masks[next_component];
            let exact = (0..other.plan.component_count)
                .find(|component| other.plan.component_masks[*component] == next_mask);
            let source = exact.or_else(|| {
                (0..other.plan.component_count)
                    .find(|component| other.plan.component_masks[*component] & next_mask != 0)
            });
            if let Some(source) = source {
                *preserved_worker = other.component_worker[source];
            }
        }
        for (component, preserved_worker) in preserved
            .iter_mut()
            .enumerate()
            .take(self.plan.component_count)
        {
            if *preserved_worker == INVALID_WORKER {
                *preserved_worker = self.component_worker[component];
            }
        }
        self.component_worker = preserved;
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            if self.plan.slot_component[slot] == INVALID_COMPONENT_ID {
                self.slot_worker[slot] = other.slot_worker[slot];
            } else if let Some(worker) = self.worker_for_component(self.plan.slot_component[slot]) {
                self.slot_worker[slot] = worker as u8;
            }
        }
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
        let mut worker_synth = [0_usize; WORKER_COUNT];
        let mut worker_sample = [0_usize; WORKER_COUNT];
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let component = self.plan.slot_component[slot];
            let worker = self.worker_for_slot(slot);
            let Some(worker) = worker else {
                return false;
            };
            worker_synth[worker] = worker_synth[worker].saturating_add(synth_counts[slot]);
            worker_sample[worker] = worker_sample[worker].saturating_add(sample_counts[slot]);
            if component == INVALID_COMPONENT_ID {
                continue;
            }
            if usize::from(component) >= self.plan.component_count {
                return false;
            }
            component_synth[component as usize] =
                component_synth[component as usize].saturating_add(synth_counts[slot]);
            component_sample[component as usize] =
                component_sample[component as usize].saturating_add(sample_counts[slot]);
        }
        (0..self.plan.component_count).all(|component| {
            component_synth[component] <= SYNTH_VOICE_LANE_CAPACITY
                && component_sample[component] <= SAMPLE_VOICE_LANE_CAPACITY
        }) && worker_synth
            .into_iter()
            .all(|count| count <= SYNTH_VOICE_LANE_CAPACITY)
            && worker_sample
                .into_iter()
                .all(|count| count <= SAMPLE_VOICE_LANE_CAPACITY)
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
    slot_worker: [u8; INSTRUMENT_SLOT_COUNT],
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
            slot_worker: [INVALID_WORKER; INSTRUMENT_SLOT_COUNT],
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
        self.slot_worker.fill(INVALID_WORKER);
        true
    }

    fn assign_workers(
        &mut self,
        plan: RoutingTreePlan,
        _instrument_kinds: [InstrumentKind; INSTRUMENT_SLOT_COUNT],
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
            let synth_count = synth_counts[slot];
            let sample_count = sample_counts[slot];
            component_synth[component as usize] =
                component_synth[component as usize].saturating_add(synth_count);
            component_sample[component as usize] =
                component_sample[component as usize].saturating_add(sample_count);
            let Some(synth_cost) = cost_units(synth_count, SOURCE_WORKER_SYNTH_COST_UNITS) else {
                return false;
            };
            let Some(sample_cost) = cost_units(sample_count, SOURCE_WORKER_SAMPLE_COST_UNITS)
            else {
                return false;
            };
            let Some(total) = component_cost[component as usize]
                .checked_add(synth_cost)
                .and_then(|total| total.checked_add(sample_cost))
            else {
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
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let component = plan.slot_component[slot];
            if component != INVALID_COMPONENT_ID {
                self.slot_worker[slot] = self.component_worker[component as usize];
                continue;
            }
            let Some(synth_cost) = cost_units(synth_counts[slot], SOURCE_WORKER_SYNTH_COST_UNITS)
            else {
                return false;
            };
            let Some(sample_cost) =
                cost_units(sample_counts[slot], SOURCE_WORKER_SAMPLE_COST_UNITS)
            else {
                return false;
            };
            let worker = if projected[1] < projected[0] { 1 } else { 0 };
            let Some(total) = projected[worker]
                .checked_add(synth_cost)
                .and_then(|total| total.checked_add(sample_cost))
            else {
                return false;
            };
            if total > SOURCE_WORKER_MAX_COST_UNITS {
                return false;
            }
            projected[worker] = total;
            self.slot_worker[slot] = worker as u8;
        }
        self.plan = plan;
        true
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn assignment(&self) -> RoutingTreeAssignment {
        RoutingTreeAssignment {
            plan: self.plan,
            component_worker: self.component_worker,
            slot_worker: self.slot_worker,
        }
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn assignment_for_engine(engine: &SynthEngine) -> Option<RoutingTreeAssignment> {
        let assignment = Self::assignment_for_engine_unvalidated(engine)?;
        assignment.validate_engine(engine).then_some(assignment)
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
        let component = self.plan.slot_component.get(slot).copied()?;
        if component != INVALID_COMPONENT_ID {
            return self.worker_for_component(component);
        }
        match self.slot_worker.get(slot).copied()? {
            worker if usize::from(worker) < WORKER_COUNT => Some(usize::from(worker)),
            _ => None,
        }
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
        self.slot_worker = [INVALID_WORKER; INSTRUMENT_SLOT_COUNT];
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
