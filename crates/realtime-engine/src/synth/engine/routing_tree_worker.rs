use super::bus_chain_owner::{BusChainFrameOutput, BusChainOwner};
use super::render_plan::RenderPlanRoute;
use super::render_routing::{render_bus_stereo_output, FxBusOutputSpreadState};
use super::routing_tree_executor::RoutingTreeAssignment;
use super::routing_tree_plan::RoutingTreePlan;
use super::source_lane_renderer::{
    render_sample_partition, render_synth_partition, SampleSourceContext, SynthSourceContext,
};
use super::source_worker_lifecycle::OwnerEnvelope;
use super::source_worker_protocol::WorkStamp;
use super::SynthEngine;
use crate::synth::dsp_config::BusIdleThreshold;
use crate::synth::fx_params::DuckSource;
use crate::synth::types::{BUS_COUNT, BUS_SLOTS_PER_BUS, INSTRUMENT_SLOT_COUNT};

pub(super) const ROUTING_TREE_WORKER_COUNT: usize = 2;

pub(super) struct RoutingTreeWorkerScratch {
    pub(super) slot_out: [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    pub(super) bus_input: [Vec<f32>; BUS_COUNT],
}

impl RoutingTreeWorkerScratch {
    pub(super) fn new() -> Self {
        Self {
            slot_out: std::array::from_fn(|_| vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES]),
            bus_input: std::array::from_fn(|_| vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES]),
        }
    }

    fn prepare(&mut self, frames: usize) -> bool {
        if frames > super::BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        for output in &mut self.slot_out {
            output[..frames].fill(0.0);
        }
        for input in &mut self.bus_input {
            input[..frames].fill(0.0);
        }
        true
    }
}

pub(super) struct RoutingTreeOutputBlock {
    pub(super) left: Vec<f32>,
    pub(super) right: Vec<f32>,
    pub(super) source_active: Vec<bool>,
    pub(super) bus_active: Vec<bool>,
    pub(super) active_synth_voices: usize,
    pub(super) active_sample_voices: usize,
    pub(super) active_bus_fx_slots: usize,
}

impl RoutingTreeOutputBlock {
    pub(super) fn new() -> Self {
        Self {
            left: vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES],
            right: vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES],
            source_active: vec![false; super::BLOCK_SLOT_SCRATCH_FRAMES],
            bus_active: vec![false; super::BLOCK_SLOT_SCRATCH_FRAMES],
            active_synth_voices: 0,
            active_sample_voices: 0,
            active_bus_fx_slots: 0,
        }
    }

    pub(super) fn prepare(&mut self, frames: usize) -> bool {
        if frames > super::BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        self.left[..frames].fill(0.0);
        self.right[..frames].fill(0.0);
        self.source_active[..frames].fill(false);
        self.bus_active[..frames].fill(false);
        self.active_synth_voices = 0;
        self.active_sample_voices = 0;
        self.active_bus_fx_slots = 0;
        true
    }
}

pub(super) struct RoutingTreeOwnerData {
    pub(super) scratch: RoutingTreeWorkerScratch,
    pub(super) output: RoutingTreeOutputBlock,
}

impl RoutingTreeOwnerData {
    pub(super) fn new() -> Self {
        Self {
            scratch: RoutingTreeWorkerScratch::new(),
            output: RoutingTreeOutputBlock::new(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct RoutingTreeWorkerContext {
    pub(super) slot_worker: [u8; INSTRUMENT_SLOT_COUNT],
    pub(super) bus_worker: [u8; BUS_COUNT],
    pub(super) slot_route: [RenderPlanRoute; INSTRUMENT_SLOT_COUNT],
    pub(super) slot_volume: [f32; INSTRUMENT_SLOT_COUNT],
    pub(super) slot_pan_gains: [(f32, f32); INSTRUMENT_SLOT_COUNT],
    pub(super) bus_pan_pos: [usize; BUS_COUNT],
    pub(super) bus_pan_gains: [(f32, f32); BUS_COUNT],
    pub(super) bus_volume: [f32; BUS_COUNT],
    pub(super) pan_positions: usize,
    pub(super) bus_count: usize,
    pub(super) sample_rate: u32,
    pub(super) synth_context: SynthSourceContext,
    pub(super) bus_idle_threshold: BusIdleThreshold,
    pub(super) fx_activity_hold_frames: u32,
}

impl RoutingTreeWorkerContext {
    pub(super) fn from_engine(
        engine: &SynthEngine,
        assignment: &RoutingTreeAssignment,
    ) -> Option<Self> {
        let bus_count = engine.bus_pan_pos.len();
        if bus_count > BUS_COUNT
            || assignment.plan != RoutingTreePlan::from_render_plan(&engine.render_plan)
        {
            return None;
        }
        Some(Self {
            slot_worker: std::array::from_fn(|slot| {
                assignment
                    .worker_for_slot(slot)
                    .map_or(u8::MAX, |worker| worker as u8)
            }),
            bus_worker: std::array::from_fn(|bus| {
                assignment
                    .worker_for_bus(bus)
                    .map_or(u8::MAX, |worker| worker as u8)
            }),
            slot_route: std::array::from_fn(|slot| engine.render_plan.instrument_slots[slot].route),
            slot_volume: engine.slot_volume,
            slot_pan_gains: engine.slot_pan_gains,
            bus_pan_pos: std::array::from_fn(|bus| {
                engine.bus_pan_pos.get(bus).copied().unwrap_or(0)
            }),
            bus_pan_gains: std::array::from_fn(|bus| {
                engine
                    .bus_pan_gains_cache
                    .get(bus)
                    .copied()
                    .unwrap_or((1.0, 1.0))
            }),
            bus_volume: std::array::from_fn(|bus| {
                engine.bus_volume.get(bus).copied().unwrap_or(1.0)
            }),
            pan_positions: engine.pan_positions,
            bus_count,
            sample_rate: engine.sample_rate,
            synth_context: engine.synth_source_context(),
            bus_idle_threshold: engine.dsp_config.bus_idle_threshold,
            fx_activity_hold_frames: engine.fx_activity_hold_frames,
        })
    }
}

pub(super) fn render_owner(
    owner: &mut OwnerEnvelope,
    context: RoutingTreeWorkerContext,
    stamp: WorkStamp,
) -> Result<u16, ()> {
    if owner.parity >= ROUTING_TREE_WORKER_COUNT
        || owner.runtime_generation != stamp.runtime_generation
        || owner.partitions.synth.parity() != owner.parity
        || owner.partitions.sample.parity() != owner.parity
        || owner.partitions.synth.render_lane_count > owner.partitions.synth.render_lanes.len()
        || owner.partitions.sample.render_lane_count > owner.partitions.sample.render_lanes.len()
        || context.bus_count > BUS_COUNT
    {
        return Err(());
    }
    if owner.routing_tree.is_none() {
        return Err(());
    }
    for carrier in owner.bus_carriers.iter_mut().flatten() {
        if carrier.owner.is_some() && !carrier.scratch.prepare(stamp.frames) {
            return Err(());
        }
    }
    render_sample_partition(
        &mut owner.partitions.sample,
        stamp.frames,
        SampleSourceContext {
            sample_rate: context.sample_rate,
        },
        &mut owner.scratch.sample,
    );
    render_synth_partition(
        &mut owner.partitions.synth,
        stamp.frames,
        stamp.base_sample_clock,
        &context.synth_context,
        &mut owner.scratch.synth,
    );
    if !valid_source_residency(owner, &context) {
        return Err(());
    }
    let Some(mut routing) = owner.routing_tree.take() else {
        return Err(());
    };
    let result = if !routing.scratch.prepare(stamp.frames) || !routing.output.prepare(stamp.frames)
    {
        Err(())
    } else {
        reduce_sources(owner, &mut routing, stamp.frames);
        stage_components(owner, &mut routing, &context, stamp.frames)
    };
    owner.routing_tree = Some(routing);
    result
}

fn valid_source_residency(owner: &OwnerEnvelope, context: &RoutingTreeWorkerContext) -> bool {
    for source in [&owner.scratch.synth, &owner.scratch.sample] {
        for (local_lane, slot) in source.slots.iter().copied().enumerate() {
            if source.rendered_frames[local_lane] == 0
                || slot == super::source_lane_renderer::INVALID_INSTRUMENT_SLOT
            {
                continue;
            }
            let slot = slot as usize;
            if slot >= INSTRUMENT_SLOT_COUNT || context.slot_worker[slot] != owner.parity as u8 {
                return false;
            }
        }
    }
    true
}

fn reduce_sources(owner: &OwnerEnvelope, routing: &mut RoutingTreeOwnerData, frames: usize) {
    for source in [&owner.scratch.sample, &owner.scratch.synth] {
        for local_lane in 0..source.slots.len() {
            let slot = source.slots[local_lane];
            if slot == super::source_lane_renderer::INVALID_INSTRUMENT_SLOT {
                continue;
            }
            let slot = slot as usize;
            if slot >= INSTRUMENT_SLOT_COUNT {
                continue;
            }
            let rendered_frames = source.rendered_frames[local_lane].min(frames);
            for frame in 0..rendered_frames {
                routing.scratch.slot_out[slot][frame] += source.samples[local_lane][frame];
            }
        }
    }
}

fn stage_components(
    owner: &mut OwnerEnvelope,
    routing: &mut RoutingTreeOwnerData,
    context: &RoutingTreeWorkerContext,
    frames: usize,
) -> Result<u16, ()> {
    let mut executed_cost = 0_u16;
    for frame in 0..frames {
        let mut raw_slots = [0.0_f32; INSTRUMENT_SLOT_COUNT];
        let source_active = owner
            .scratch
            .sample
            .rendered_frames
            .iter()
            .chain(owner.scratch.synth.rendered_frames.iter())
            .any(|prefix| *prefix > frame);
        for (slot, raw) in raw_slots.iter_mut().enumerate() {
            *raw = routing.scratch.slot_out[slot][frame];
            let worker = context.slot_worker[slot];
            if worker == owner.parity as u8 {
                let sample = *raw * context.slot_volume[slot];
                match context.slot_route[slot] {
                    RenderPlanRoute::Direct => {
                        routing.output.left[frame] += sample * context.slot_pan_gains[slot].0;
                        routing.output.right[frame] += sample * context.slot_pan_gains[slot].1;
                    }
                    RenderPlanRoute::Bus(bus) => {
                        if bus >= context.bus_count || context.bus_worker[bus] != owner.parity as u8
                        {
                            return Err(());
                        }
                        routing.scratch.bus_input[bus][frame] += sample;
                    }
                }
            } else if *raw != 0.0 && context.slot_worker[slot] != u8::MAX {
                return Err(());
            }
        }
        routing.output.source_active[frame] = source_active;
        let bus_snapshot: [f32; BUS_COUNT] =
            std::array::from_fn(|bus| routing.scratch.bus_input[bus][frame]);
        for bus in 0..context.bus_count {
            if context.bus_worker[bus] != owner.parity as u8 {
                continue;
            }
            let Some(carrier) = owner.bus_carriers[bus].as_mut() else {
                return Err(());
            };
            if carrier.owner.is_none() {
                continue;
            }
            carrier.scratch.input[frame] = routing.scratch.bus_input[bus][frame];
            for slot in 0..BUS_SLOTS_PER_BUS {
                let source = match carrier.owner.as_ref().map(|chain| chain.slot_params[slot]) {
                    Some(super::super::fx_params::FxBusParams::Duck { source, .. }) => source,
                    _ => continue,
                };
                carrier.scratch.resolved_duck[slot][frame] = match source {
                    DuckSource::Instrument(slot) if slot < INSTRUMENT_SLOT_COUNT => raw_slots[slot],
                    DuckSource::Bus(source_bus) if source_bus < context.bus_count => {
                        if context.bus_worker[source_bus] != owner.parity as u8 {
                            return Err(());
                        }
                        bus_snapshot[source_bus]
                    }
                    _ => return Err(()),
                };
            }
        }
    }
    for bus in 0..context.bus_count {
        if context.bus_worker[bus] != owner.parity as u8 {
            continue;
        }
        let Some(carrier) = owner.bus_carriers[bus].as_mut() else {
            return Err(());
        };
        let Some(chain) = carrier.owner.as_mut() else {
            continue;
        };
        if carrier.routing_tree_spread_state.is_none() {
            carrier.routing_tree_spread_state =
                Some(FxBusOutputSpreadState::new(context.sample_rate));
        }
        let bus_cost = chain
            .process_block(
                &mut carrier.scratch,
                frames,
                context.sample_rate,
                context.bus_idle_threshold,
                context.fx_activity_hold_frames,
            )
            .map_err(|_| ())?;
        executed_cost = executed_cost.saturating_add(bus_cost);
        let spread_state = carrier
            .routing_tree_spread_state
            .as_mut()
            .expect("spread state");
        for frame in 0..frames {
            if !carrier.scratch.executed || frame >= carrier.scratch.processed_prefix {
                continue;
            }
            let output = BusChainFrameOutput {
                mono: carrier.scratch.mono_output[frame],
                auto_pan_pos: (!carrier.scratch.auto_pan_pos[frame].is_nan())
                    .then_some(carrier.scratch.auto_pan_pos[frame]),
                spread: carrier.scratch.spread,
            };
            let (left, right) = render_bus_stereo_output(
                output,
                spread_state,
                Some(context.bus_pan_pos[bus]),
                context.pan_positions,
                context.bus_pan_gains[bus],
                context.bus_volume[bus],
            );
            routing.output.left[frame] += left;
            routing.output.right[frame] += right;
        }
    }
    routing.output.bus_active[..frames].fill(false);
    for bus in 0..context.bus_count {
        if context.bus_worker[bus] == owner.parity as u8 {
            routing.output.bus_active[..frames]
                .iter_mut()
                .for_each(|active| {
                    *active |= owner.bus_carriers[bus]
                        .as_ref()
                        .and_then(|carrier| carrier.owner.as_ref())
                        .is_some_and(BusChainOwner::is_active);
                });
        }
    }
    routing.output.active_synth_voices = owner.partitions.synth.active_count();
    routing.output.active_sample_voices = owner.partitions.sample.active_count();
    routing.output.active_bus_fx_slots = owner
        .bus_carriers
        .iter()
        .filter_map(|carrier| carrier.as_ref().and_then(|carrier| carrier.owner.as_ref()))
        .map(|chain| chain.active_slot_count)
        .sum();
    Ok(executed_cost)
}
