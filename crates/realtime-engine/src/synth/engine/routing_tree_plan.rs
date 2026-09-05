use super::super::fx_params::{DuckSource, FxKind};
use super::super::types::{BUS_COUNT, BUS_SLOTS_PER_BUS, INSTRUMENT_SLOT_COUNT};
use super::render_plan::{RenderPlan, RenderPlanRoute};

pub(super) const INVALID_COMPONENT_ID: u8 = u8::MAX;

pub(super) const ROUTING_NODE_COUNT: usize = INSTRUMENT_SLOT_COUNT + BUS_COUNT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RoutingTreePlan {
    pub(super) generation: u64,
    pub(super) slot_component: [u8; INSTRUMENT_SLOT_COUNT],
    pub(super) bus_component: [u8; BUS_COUNT],
    pub(super) component_masks: [u16; ROUTING_NODE_COUNT],
    pub(super) component_count: usize,
}

impl RoutingTreePlan {
    pub(super) fn same_structure(&self, other: &Self) -> bool {
        self.slot_component == other.slot_component
            && self.bus_component == other.bus_component
            && self.component_masks == other.component_masks
            && self.component_count == other.component_count
    }

    pub(super) fn from_render_plan(render_plan: &RenderPlan) -> Self {
        let configured_bus_count = render_plan.bus_fx_slots.len().min(BUS_COUNT);
        let mut nodes = RoutingNodes::new();
        for bus in 0..configured_bus_count {
            nodes.active[bus_node(bus)] = true;
        }
        for (slot, instrument) in render_plan.instrument_slots.iter().enumerate() {
            if !instrument.occupied {
                continue;
            }
            nodes.active[slot] = true;
            if let RenderPlanRoute::Bus(bus) = instrument.route {
                if bus < configured_bus_count {
                    nodes.union(slot, bus_node(bus));
                }
            }
        }
        for bus in 0..configured_bus_count {
            for slot in render_plan.bus_fx_slots[bus].iter().take(BUS_SLOTS_PER_BUS) {
                if slot.kind != FxKind::Duck {
                    continue;
                }
                let Some(source) = slot.duck_source else {
                    continue;
                };
                let source_node = match source {
                    DuckSource::Instrument(instrument)
                        if instrument < INSTRUMENT_SLOT_COUNT && nodes.active[instrument] =>
                    {
                        instrument
                    }
                    DuckSource::Bus(source_bus) if source_bus < configured_bus_count => {
                        bus_node(source_bus)
                    }
                    _ => continue,
                };
                nodes.union(bus_node(bus), source_node);
            }
        }
        nodes.into_plan(render_plan.generation)
    }
}

struct RoutingNodes {
    parent: [u8; ROUTING_NODE_COUNT],
    active: [bool; ROUTING_NODE_COUNT],
}

impl RoutingNodes {
    fn new() -> Self {
        Self {
            parent: std::array::from_fn(|node| node as u8),
            active: [false; ROUTING_NODE_COUNT],
        }
    }

    fn find(&mut self, node: usize) -> usize {
        let parent = self.parent[node] as usize;
        if parent == node {
            return node;
        }
        let root = self.find(parent);
        self.parent[node] = root as u8;
        root
    }

    fn union(&mut self, left: usize, right: usize) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return;
        }
        if left_root < right_root {
            self.parent[right_root] = left_root as u8;
        } else {
            self.parent[left_root] = right_root as u8;
        }
    }

    fn into_plan(mut self, generation: u64) -> RoutingTreePlan {
        let mut plan = RoutingTreePlan {
            generation,
            slot_component: [INVALID_COMPONENT_ID; INSTRUMENT_SLOT_COUNT],
            bus_component: [INVALID_COMPONENT_ID; BUS_COUNT],
            component_masks: [0; ROUTING_NODE_COUNT],
            component_count: 0,
        };
        let mut root_components = [INVALID_COMPONENT_ID; ROUTING_NODE_COUNT];
        for node in 0..ROUTING_NODE_COUNT {
            if !self.active[node] {
                continue;
            }
            let root = self.find(node);
            let component = if root_components[root] == INVALID_COMPONENT_ID {
                let component = plan.component_count as u8;
                root_components[root] = component;
                plan.component_count += 1;
                component
            } else {
                root_components[root]
            };
            plan.component_masks[component as usize] |= 1_u16 << node;
            if node < INSTRUMENT_SLOT_COUNT {
                plan.slot_component[node] = component;
            } else {
                plan.bus_component[node - INSTRUMENT_SLOT_COUNT] = component;
            }
        }
        plan
    }
}

fn bus_node(bus: usize) -> usize {
    INSTRUMENT_SLOT_COUNT + bus
}

const _: () = assert!(ROUTING_NODE_COUNT <= u16::BITS as usize);
