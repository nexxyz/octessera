use super::render_samples::render_preview_sample_voices_block_into;
use super::retired_state::PREVIEW_AUDITION_SLOTS;
use super::routing_tree_executor::RoutingTreeAssignment;
use super::routing_tree_plan::RoutingTreePlan;
use super::routing_tree_source_bank::RoutingTreeSourceBank;
use super::routing_tree_source_renderer::{
    render_routing_tree_sources, RoutingTreeSourceBlockScratch,
};
use super::source_lane_renderer::SynthSourceContext;
use super::source_worker_lifecycle::OwnerEnvelope;
use super::source_worker_protocol::WorkStamp;
use super::support::{MomentaryFxState, PreviewSampleVoice};
use super::SynthEngine;
use crate::synth::dsp_config::BusIdleThreshold;
use crate::synth::types::{
    BUS_COUNT, BUS_SLOTS_PER_BUS, INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY,
    SYNTH_VOICE_LANE_CAPACITY,
};

pub(super) const ROUTING_TREE_WORKER_COUNT: usize = 2;
pub(super) const ROUTING_TREE_MAX_COST_UNITS: u16 = (SYNTH_VOICE_LANE_CAPACITY
    * super::source_worker_load::SOURCE_WORKER_SYNTH_COST_UNITS as usize
    + (SAMPLE_VOICE_LANE_CAPACITY + PREVIEW_AUDITION_SLOTS)
        * super::source_worker_load::SOURCE_WORKER_SAMPLE_COST_UNITS as usize
    + BUS_COUNT * BUS_SLOTS_PER_BUS * super::bus_chain_owner::BUS_CHAIN_SLOT_COST_UNITS as usize)
    as u16;

pub(super) struct RoutingTreeWorkerScratch {
    pub(super) slot_out: [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    pub(super) bus_input: [Vec<f32>; BUS_COUNT],
    pub(super) source: RoutingTreeSourceBlockScratch,
    pub(super) preview_active: Vec<bool>,
    pub(super) momentary_active: Vec<bool>,
}

impl RoutingTreeWorkerScratch {
    pub(super) fn new() -> Self {
        Self {
            slot_out: std::array::from_fn(|_| vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES]),
            bus_input: std::array::from_fn(|_| vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES]),
            source: RoutingTreeSourceBlockScratch::new(),
            preview_active: vec![false; super::BLOCK_SLOT_SCRATCH_FRAMES],
            momentary_active: vec![false; super::BLOCK_SLOT_SCRATCH_FRAMES],
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
        self.preview_active[..frames].fill(false);
        self.momentary_active[..frames].fill(false);
        self.source.prepare(frames)
    }
}

pub(super) struct RoutingTreeOutputBlock {
    pub(super) left: Vec<f32>,
    pub(super) right: Vec<f32>,
    pub(super) source_active: Vec<bool>,
    pub(super) bus_active: Vec<bool>,
    pub(super) active_synth_voices: usize,
    pub(super) active_sample_voices: usize,
    pub(super) active_preview_sample_voices: usize,
    pub(super) active_momentary_fx: usize,
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
            active_preview_sample_voices: 0,
            active_momentary_fx: 0,
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
        self.active_preview_sample_voices = 0;
        self.active_momentary_fx = 0;
        self.active_bus_fx_slots = 0;
        true
    }
}

pub(super) struct RoutingTreeOwnerData {
    pub(super) scratch: RoutingTreeWorkerScratch,
    pub(super) output: RoutingTreeOutputBlock,
    pub(super) source_bank: Option<Box<RoutingTreeSourceBank>>,
    pub(super) preview_sample_voices: [Option<PreviewSampleVoice>; PREVIEW_AUDITION_SLOTS],
    pub(super) preview_sample_orders: [u64; PREVIEW_AUDITION_SLOTS],
    pub(super) momentary_fx: Vec<MomentaryFxState>,
    pub(super) retired_preview_samples: [Option<PreviewSampleVoice>; PREVIEW_AUDITION_SLOTS],
    pub(super) retired_momentary_fx: [Option<MomentaryFxState>; PREVIEW_AUDITION_SLOTS],
}

impl RoutingTreeOwnerData {
    pub(super) fn new(source_bank: Box<RoutingTreeSourceBank>) -> Self {
        Self {
            scratch: RoutingTreeWorkerScratch::new(),
            output: RoutingTreeOutputBlock::new(),
            source_bank: Some(source_bank),
            preview_sample_voices: std::array::from_fn(|_| None),
            preview_sample_orders: [0; PREVIEW_AUDITION_SLOTS],
            momentary_fx: Vec::with_capacity(super::control::MAX_MOMENTARY_FX),
            retired_preview_samples: std::array::from_fn(|_| None),
            retired_momentary_fx: std::array::from_fn(|_| None),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct RoutingTreeWorkerContext {
    pub(super) slot_worker: [u8; INSTRUMENT_SLOT_COUNT],
    pub(super) bus_worker: [u8; BUS_COUNT],
    pub(super) slot_route: [super::render_plan::RenderPlanRoute; INSTRUMENT_SLOT_COUNT],
    pub(super) slot_volume: [f32; INSTRUMENT_SLOT_COUNT],
    pub(super) slot_pan_gains: [(f32, f32); INSTRUMENT_SLOT_COUNT],
    pub(super) bus_pan_pos: [usize; BUS_COUNT],
    pub(super) bus_pan_gains: [(f32, f32); BUS_COUNT],
    pub(super) bus_volume: [f32; BUS_COUNT],
    pub(super) pan_positions: usize,
    pub(super) bus_count: usize,
    pub(super) sample_rate: u32,
    pub(super) synth_context: SynthSourceContext,
    pub(super) sample_filter: [(f32, f32); INSTRUMENT_SLOT_COUNT],
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
            sample_filter: std::array::from_fn(|slot| {
                engine
                    .sample_banks
                    .get(slot)
                    .map(|bank| (bank.filter_cutoff_hz, bank.filter_resonance))
                    .unwrap_or((8000.0, 20.0))
            }),
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
    for carrier in owner.bus_carriers.iter_mut().flatten() {
        if carrier.owner.is_some() && !carrier.scratch.prepare(stamp.frames) {
            return Err(());
        }
    }
    let Some(mut routing) = owner.routing_tree.take() else {
        return Err(());
    };
    let result = if !routing.scratch.prepare(stamp.frames) || !routing.output.prepare(stamp.frames)
    {
        Err(())
    } else {
        let Some(source_bank) = routing.source_bank.as_mut() else {
            return Err(());
        };
        render_routing_tree_sources(
            source_bank,
            stamp.frames,
            stamp.base_sample_clock,
            &context.synth_context,
            context.sample_rate,
            &mut routing.scratch.source,
        );
        if !valid_source_residency(owner, &routing, &context) {
            Err(())
        } else {
            let completed = render_preview_sample_voices_block_into(
                &mut routing.preview_sample_voices,
                &context.sample_filter,
                context.sample_rate,
                stamp.frames,
                &mut routing.scratch.slot_out,
                &mut routing.scratch.preview_active,
            );
            for voice in completed.into_iter().flatten() {
                super::retired_state::store_retired_preview(
                    &mut routing.retired_preview_samples,
                    voice,
                );
            }
            reduce_sources(&mut routing, stamp.frames);
            super::routing_tree_component_renderer::stage_components(
                owner,
                &mut routing,
                &context,
                stamp.frames,
            )
        }
    };
    owner.routing_tree = Some(routing);
    result
}

fn valid_source_residency(
    owner: &OwnerEnvelope,
    routing: &RoutingTreeOwnerData,
    context: &RoutingTreeWorkerContext,
) -> bool {
    let Some(source) = routing.source_bank.as_ref() else {
        return false;
    };
    source
        .synth
        .iter()
        .filter(|voice| voice.active)
        .all(|voice| {
            let slot = voice.instrument_slot as usize;
            slot < INSTRUMENT_SLOT_COUNT
                && voice.canonical_lane.is_some()
                && context.slot_worker[slot] == owner.parity as u8
        })
        && source
            .sample
            .iter()
            .filter(|voice| voice.active)
            .all(|voice| {
                let slot = voice.instrument_slot as usize;
                slot < INSTRUMENT_SLOT_COUNT
                    && voice.canonical_lane.is_some()
                    && context.slot_worker[slot] == owner.parity as u8
            })
}

fn reduce_sources(routing: &mut RoutingTreeOwnerData, frames: usize) {
    let Some(source) = routing.source_bank.as_ref() else {
        return;
    };
    for lane in 0..source.sample.len() {
        let slot = source.sample[lane].instrument_slot as usize;
        if slot >= INSTRUMENT_SLOT_COUNT {
            continue;
        }
        let rendered_frames = routing.scratch.source.sample_rendered_frames[lane].min(frames);
        for frame in 0..rendered_frames {
            routing.scratch.slot_out[slot][frame] +=
                routing.scratch.source.sample_samples[lane][frame];
        }
    }
    for lane in 0..source.synth.len() {
        let slot = source.synth[lane].instrument_slot as usize;
        if slot >= INSTRUMENT_SLOT_COUNT {
            continue;
        }
        let rendered_frames = routing.scratch.source.synth_rendered_frames[lane].min(frames);
        for frame in 0..rendered_frames {
            routing.scratch.slot_out[slot][frame] +=
                routing.scratch.source.synth_samples[lane][frame];
        }
    }
}
