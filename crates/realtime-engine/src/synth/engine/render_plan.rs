use super::super::fx_params::{duck_source_from_config, DuckSource, FxKind};
use super::super::types::{
    FxBusSlotConfig, InstrumentSlotConfig, InstrumentsConfig, BUS_SLOTS_PER_BUS,
    GLOBAL_FX_SLOT_COUNT, INSTRUMENT_SLOT_COUNT,
};
use super::support::{parse_instrument_kind, parse_route, InstrumentKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RenderPlanRoute {
    Direct,
    Bus(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PreparedInstrumentTopology {
    pub(super) kind: InstrumentKind,
    pub(super) occupied: bool,
    pub(super) route: Option<RenderPlanRoute>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RenderPlanInstrumentSlot {
    pub(super) kind: InstrumentKind,
    pub(super) occupied: bool,
    pub(super) route: RenderPlanRoute,
}

impl RenderPlanInstrumentSlot {
    fn empty() -> Self {
        Self {
            kind: InstrumentKind::Synth,
            occupied: false,
            route: RenderPlanRoute::Direct,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RenderPlanFxSlot {
    pub(super) kind: FxKind,
    pub(super) duck_source: Option<DuckSource>,
}

impl RenderPlanFxSlot {
    fn none() -> Self {
        Self {
            kind: FxKind::None,
            duck_source: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RenderPlan {
    pub(super) generation: u64,
    pub(super) instrument_slots: [RenderPlanInstrumentSlot; INSTRUMENT_SLOT_COUNT],
    pub(super) bus_fx_slots: Vec<[RenderPlanFxSlot; BUS_SLOTS_PER_BUS]>,
    pub(super) master_fx_slots: Vec<RenderPlanFxSlot>,
}

impl RenderPlan {
    pub(super) fn new() -> Self {
        Self {
            generation: 0,
            instrument_slots: [RenderPlanInstrumentSlot::empty(); INSTRUMENT_SLOT_COUNT],
            bus_fx_slots: Vec::new(),
            master_fx_slots: Vec::new(),
        }
    }

    pub(super) fn from_config(config: &InstrumentsConfig) -> Self {
        let mut plan = Self {
            generation: 0,
            instrument_slots: [RenderPlanInstrumentSlot::empty(); INSTRUMENT_SLOT_COUNT],
            bus_fx_slots: Vec::with_capacity(
                config.mixer.as_ref().map_or(0, |mixer| mixer.buses.len()),
            ),
            master_fx_slots: Vec::new(),
        };
        for (index, slot) in config
            .instruments
            .iter()
            .take(INSTRUMENT_SLOT_COUNT)
            .enumerate()
        {
            let topology = prepared_instrument_topology(slot);
            plan.instrument_slots[index] = RenderPlanInstrumentSlot {
                kind: topology.kind,
                occupied: topology.occupied,
                route: topology.route.unwrap_or(RenderPlanRoute::Direct),
            };
        }
        if let Some(mixer) = config.mixer.as_ref() {
            for bus in &mixer.buses {
                plan.bus_fx_slots.push(std::array::from_fn(|index| {
                    bus.slots
                        .get(index)
                        .map_or_else(RenderPlanFxSlot::none, render_plan_fx_slot)
                }));
            }
            if let Some(master) = mixer.master.as_ref() {
                plan.master_fx_slots = master
                    .slots
                    .iter()
                    .take(GLOBAL_FX_SLOT_COUNT)
                    .map(render_plan_fx_slot)
                    .collect();
            }
        }
        plan
    }

    pub(super) fn install_instrument_slot(
        &mut self,
        index: usize,
        topology: PreparedInstrumentTopology,
    ) {
        let Some(current) = self.instrument_slots.get(index).copied() else {
            return;
        };
        let next = RenderPlanInstrumentSlot {
            kind: topology.kind,
            occupied: topology.occupied,
            route: topology.route.unwrap_or(current.route),
        };
        if current != next {
            self.instrument_slots[index] = next;
            self.bump_generation();
        }
    }

    pub(super) fn install_bus_fx_slot(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        topology: RenderPlanFxSlot,
    ) {
        let Some(slot) = self
            .bus_fx_slots
            .get_mut(bus_index)
            .and_then(|slots| slots.get_mut(slot_index))
        else {
            return;
        };
        if *slot != topology {
            *slot = topology;
            self.bump_generation();
        }
    }

    pub(super) fn install_master_fx_slot(&mut self, slot_index: usize, topology: RenderPlanFxSlot) {
        let Some(slot) = self.master_fx_slots.get_mut(slot_index) else {
            return;
        };
        if *slot != topology {
            *slot = topology;
            self.bump_generation();
        }
    }

    pub(super) fn install_complete(&mut self, mut next: RenderPlan) -> RenderPlan {
        if self.same_topology(&next) {
            next.generation = self.generation;
            return next;
        }
        next.generation = self.generation.wrapping_add(1);
        std::mem::replace(self, next)
    }

    fn same_topology(&self, other: &RenderPlan) -> bool {
        self.instrument_slots == other.instrument_slots
            && self.bus_fx_slots == other.bus_fx_slots
            && self.master_fx_slots == other.master_fx_slots
    }

    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}

pub(super) fn prepared_instrument_topology(
    slot: &InstrumentSlotConfig,
) -> PreparedInstrumentTopology {
    let kind = parse_instrument_kind(&slot.kind);
    let route = slot
        .mixer
        .as_ref()
        .map(|mixer| render_plan_route(parse_route(&mixer.route)));
    PreparedInstrumentTopology {
        kind,
        occupied: kind != InstrumentKind::None,
        route,
    }
}

pub(super) fn render_plan_fx_slot(config: &FxBusSlotConfig) -> RenderPlanFxSlot {
    let kind = FxKind::parse(config.kind_str()).unwrap_or(FxKind::None);
    let duck_source = matches!(kind, FxKind::Duck).then(|| duck_source_from_config(config));
    RenderPlanFxSlot { kind, duck_source }
}

fn render_plan_route(route: usize) -> RenderPlanRoute {
    if route == 0 {
        RenderPlanRoute::Direct
    } else {
        RenderPlanRoute::Bus(route - 1)
    }
}
