use super::super::dsp_config::BusIdleThreshold;
#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
use super::super::fx::process_fx_bus_slot_with_duck_source;
use super::super::fx::{fx_bus_state_matches_params, process_fx_bus_slot, FxBusState};
use super::super::fx_params::{FxBusParams, FxKind};
use super::super::types::{BUS_SLOTS_PER_BUS, INSTRUMENT_SLOT_COUNT};
#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
use super::source_worker_load::SOURCE_WORKER_MAX_COST_UNITS;

pub const BUS_CHAIN_SLOT_COST_UNITS: u16 = 4;

pub(super) const fn fx_kind_cost(kind: FxKind) -> u16 {
    match kind {
        FxKind::None => 0,
        FxKind::Duck | FxKind::Distortion | FxKind::Bitcrusher => 1,
        FxKind::Tremolo | FxKind::Delay | FxKind::Glitch | FxKind::AutoPan | FxKind::Saturator => 2,
        FxKind::Vibrato | FxKind::Chorus | FxKind::Flanger | FxKind::Reverb | FxKind::Eq => 3,
        FxKind::FilterLfo | FxKind::Wah | FxKind::Compressor | FxKind::Vinyl => {
            BUS_CHAIN_SLOT_COST_UNITS
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct BusChainFrameOutput {
    pub(super) mono: f32,
    pub(super) auto_pan_pos: Option<f32>,
    pub(super) spread: f32,
}

#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
pub(super) struct BusChainBlockScratch {
    pub(super) input: Vec<f32>,
    pub(super) resolved_duck: [Vec<f32>; BUS_SLOTS_PER_BUS],
    pub(super) mono_output: Vec<f32>,
    pub(super) auto_pan_pos: Vec<f32>,
    pub(super) active: Vec<bool>,
    pub(super) processed_prefix: usize,
    pub(super) spread: f32,
    pub(super) executed: bool,
}

#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
impl BusChainBlockScratch {
    pub(super) fn new() -> Self {
        Self {
            input: vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES],
            resolved_duck: std::array::from_fn(|_| vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES]),
            mono_output: vec![0.0; super::BLOCK_SLOT_SCRATCH_FRAMES],
            auto_pan_pos: vec![f32::NAN; super::BLOCK_SLOT_SCRATCH_FRAMES],
            active: vec![false; super::BLOCK_SLOT_SCRATCH_FRAMES],
            processed_prefix: 0,
            spread: 0.0,
            executed: false,
        }
    }

    pub(super) fn prepare(&mut self, frames: usize) -> bool {
        if frames > super::BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        self.input[..frames].fill(0.0);
        for buffer in &mut self.resolved_duck {
            buffer[..frames].fill(0.0);
        }
        self.mono_output[..frames].fill(0.0);
        self.auto_pan_pos[..frames].fill(f32::NAN);
        self.active[..frames].fill(false);
        self.processed_prefix = 0;
        self.spread = 0.0;
        self.executed = false;
        true
    }

    pub(super) fn prepare_for_render(&mut self, frames: usize) -> bool {
        if frames > super::BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        self.mono_output[..frames].fill(0.0);
        self.auto_pan_pos[..frames].fill(f32::NAN);
        self.active[..frames].fill(false);
        self.processed_prefix = 0;
        self.spread = 0.0;
        self.executed = false;
        true
    }
}

#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
pub(super) struct BusChainCarrier {
    pub(super) logical_bus_id: usize,
    pub(super) owner: Option<BusChainOwner>,
    pub(super) scratch: BusChainBlockScratch,
    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) routing_tree_spread_state: Option<super::render_routing::FxBusOutputSpreadState>,
}

#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
impl BusChainCarrier {
    pub(super) fn new(logical_bus_id: usize, owner: Option<BusChainOwner>) -> Self {
        Self {
            logical_bus_id,
            owner,
            scratch: BusChainBlockScratch::new(),
            #[cfg(feature = "routing-tree-benchmark")]
            routing_tree_spread_state: None,
        }
    }

    pub(super) fn cost_units(&self) -> u16 {
        self.owner.as_ref().map_or(0, BusChainOwner::cost_units)
    }

    pub(super) fn prepare(&mut self, frames: usize) -> bool {
        self.scratch.prepare(frames)
    }

    pub(super) fn within_worker_capacity(&self) -> bool {
        self.cost_units() <= SOURCE_WORKER_MAX_COST_UNITS
    }
}

#[derive(Clone, Debug)]
pub(super) struct BusChainSlot {
    pub(super) params: FxBusParams,
    pub(super) state: FxBusState,
    pub(super) cost: u16,
}

#[derive(Clone, Debug)]
pub(super) struct BusChainOwner {
    pub(super) logical_bus_id: usize,
    pub(super) slot_params: [FxBusParams; BUS_SLOTS_PER_BUS],
    pub(super) slot_state: [FxBusState; BUS_SLOTS_PER_BUS],
    pub(super) slot_costs: [u16; BUS_SLOTS_PER_BUS],
    pub(super) active_slot_indices: [usize; BUS_SLOTS_PER_BUS],
    pub(super) active_slot_count: usize,
    pub(super) render_hold_frames: u32,
    pub(super) quiet_frames: u32,
    pub(super) assigned_worker: Option<usize>,
}

impl BusChainOwner {
    pub(super) fn new(
        logical_bus_id: usize,
        slot_params: [FxBusParams; BUS_SLOTS_PER_BUS],
        slot_state: [FxBusState; BUS_SLOTS_PER_BUS],
        slot_costs: [u16; BUS_SLOTS_PER_BUS],
    ) -> Self {
        let (active_slot_indices, active_slot_count) = active_slots(&slot_params);
        Self {
            logical_bus_id,
            slot_params,
            slot_state,
            slot_costs,
            active_slot_indices,
            active_slot_count,
            render_hold_frames: 0,
            quiet_frames: 0,
            assigned_worker: None,
        }
    }

    pub(super) fn from_slot(logical_bus_id: usize, slot_index: usize, slot: BusChainSlot) -> Self {
        let mut owner = Self::new(
            logical_bus_id,
            std::array::from_fn(|_| FxBusParams::None),
            std::array::from_fn(|_| FxBusState::None),
            [0; BUS_SLOTS_PER_BUS],
        );
        let slot_index = slot_index.min(BUS_SLOTS_PER_BUS - 1);
        owner.slot_params[slot_index] = slot.params;
        owner.slot_state[slot_index] = slot.state;
        owner.slot_costs[slot_index] = slot.cost;
        owner.refresh_active_slots();
        owner
    }

    pub(super) fn process(
        &mut self,
        input: f32,
        slot_out: &[f32; INSTRUMENT_SLOT_COUNT],
        bus_mono_snapshot: &[f32],
        sample_rate: u32,
    ) -> BusChainFrameOutput {
        let mut processed = input;
        let mut auto_pan_pos = None;
        let mut spread = 0.0_f32;
        for slot_index in self
            .active_slot_indices
            .iter()
            .take(self.active_slot_count)
            .copied()
        {
            processed = process_fx_bus_slot(
                &self.slot_params[slot_index],
                &mut self.slot_state[slot_index],
                processed,
                slot_out,
                bus_mono_snapshot,
                sample_rate,
            );
            match (&self.slot_params[slot_index], &self.slot_state[slot_index]) {
                (
                    FxBusParams::Delay {
                        mix,
                        spread: slot_spread,
                        ..
                    },
                    _,
                ) => {
                    spread = spread.max(slot_spread * mix);
                }
                (_, FxBusState::AutoPan { pos, .. }) => {
                    auto_pan_pos = Some(pos.clamp(0.0, 1.0));
                }
                _ => {}
            }
        }
        BusChainFrameOutput {
            mono: processed,
            auto_pan_pos,
            spread,
        }
    }

    #[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
    pub(super) fn process_block(
        &mut self,
        scratch: &mut BusChainBlockScratch,
        frames: usize,
        sample_rate: u32,
        threshold: BusIdleThreshold,
        hold_frames: u32,
    ) -> Result<u16, ()> {
        if !scratch.prepare_for_render(frames) || sample_rate == 0 {
            return Err(());
        }
        let cost = self.cost_units();
        for frame in 0..frames {
            let input = scratch.input[frame];
            let active = input.abs() > 1.0e-5 || self.is_active();
            scratch.active[frame] = active;
            if !active {
                self.observe(input, 0.0, threshold, sample_rate);
                self.observe_render_hold(false, false, hold_frames);
                continue;
            }
            scratch.executed = true;
            let mut processed = input;
            let mut auto_pan_pos = None;
            for slot_index in self
                .active_slot_indices
                .iter()
                .take(self.active_slot_count)
                .copied()
            {
                processed = process_fx_bus_slot_with_duck_source(
                    &self.slot_params[slot_index],
                    &mut self.slot_state[slot_index],
                    processed,
                    scratch.resolved_duck[slot_index][frame],
                    sample_rate,
                );
                match (&self.slot_params[slot_index], &self.slot_state[slot_index]) {
                    (
                        FxBusParams::Delay {
                            mix,
                            spread: slot_spread,
                            ..
                        },
                        _,
                    ) => {
                        scratch.spread = scratch.spread.max(slot_spread * mix);
                    }
                    (_, FxBusState::AutoPan { pos, .. }) => {
                        auto_pan_pos = Some(pos.clamp(0.0, 1.0));
                    }
                    _ => {}
                }
            }
            scratch.mono_output[frame] = processed;
            scratch.auto_pan_pos[frame] = auto_pan_pos.unwrap_or(f32::NAN);
            self.observe(input, processed, threshold, sample_rate);
            let input_present = input.abs() > 1.0e-5;
            let output_present = processed.abs() > 1.0e-5;
            self.observe_render_hold(input_present, output_present, hold_frames);
            scratch.active[frame] = self.is_active();
        }
        scratch.processed_prefix = frames;
        Ok(if scratch.executed { cost } else { 0 })
    }

    pub(super) fn replace_slot(
        &mut self,
        slot_index: usize,
        params: FxBusParams,
        state: FxBusState,
        cost: u16,
    ) -> Option<BusChainSlot> {
        if slot_index >= BUS_SLOTS_PER_BUS {
            return Some(BusChainSlot {
                params,
                state,
                cost,
            });
        }
        let previous_params = self.slot_params[slot_index];
        let previous_cost = self.slot_costs[slot_index];
        let previous_state = std::mem::replace(&mut self.slot_state[slot_index], state);
        let preserved_state = fx_bus_state_matches_params(&previous_state, &params);
        let retired_state = if preserved_state {
            std::mem::replace(&mut self.slot_state[slot_index], previous_state)
        } else {
            previous_state
        };
        self.slot_params[slot_index] = params;
        self.slot_costs[slot_index] = cost;
        self.refresh_active_slots();
        if previous_params != params {
            self.reset_quiet();
        }
        if self.cost_units() == 0 {
            self.assigned_worker = None;
        }
        Some(BusChainSlot {
            params: if preserved_state {
                params
            } else {
                previous_params
            },
            state: retired_state,
            cost: if preserved_state { cost } else { previous_cost },
        })
    }

    pub(super) fn preserve_state_from(&mut self, previous: &mut Self) {
        debug_assert_eq!(self.logical_bus_id, previous.logical_bus_id);
        let same_slots = self.slot_params == previous.slot_params;
        self.assigned_worker = previous.assigned_worker;
        self.render_hold_frames = previous.render_hold_frames;
        self.quiet_frames = if same_slots { previous.quiet_frames } else { 0 };
        for slot_index in 0..BUS_SLOTS_PER_BUS {
            if fx_bus_state_matches_params(
                &previous.slot_state[slot_index],
                &self.slot_params[slot_index],
            ) {
                std::mem::swap(
                    &mut previous.slot_state[slot_index],
                    &mut self.slot_state[slot_index],
                );
            }
        }
        if self.cost_units() == 0 {
            self.assigned_worker = None;
        }
    }

    pub(super) fn cost_units(&self) -> u16 {
        self.slot_costs.iter().copied().sum()
    }

    pub(super) fn observe(
        &mut self,
        routed_input: f32,
        post_chain_output: f32,
        threshold: BusIdleThreshold,
        sample_rate: u32,
    ) {
        let amplitude = threshold.amplitude();
        if !(routed_input.is_finite()
            && post_chain_output.is_finite()
            && routed_input.abs() <= amplitude
            && post_chain_output.abs() <= amplitude)
        {
            self.reset_quiet();
            return;
        }
        let required_frames = u64::from(sample_rate).saturating_mul(250) / 1000;
        if required_frames == 0 {
            return;
        }
        self.quiet_frames = self
            .quiet_frames
            .saturating_add(1)
            .min(required_frames.min(u64::from(u32::MAX)) as u32);
        if u64::from(self.quiet_frames) >= required_frames {
            self.assigned_worker = None;
        }
    }

    pub(super) fn observe_render_hold(
        &mut self,
        input_present: bool,
        output_present: bool,
        hold_frames: u32,
    ) {
        if input_present || output_present {
            self.render_hold_frames = hold_frames;
        } else {
            self.render_hold_frames = self.render_hold_frames.saturating_sub(1);
        }
    }

    pub(super) fn reset_quiet(&mut self) {
        self.quiet_frames = 0;
    }

    pub(super) fn is_active(&self) -> bool {
        self.render_hold_frames > 0
    }

    pub(super) fn is_loud(
        routed_input: f32,
        post_chain_output: f32,
        threshold: BusIdleThreshold,
    ) -> bool {
        !(routed_input.is_finite()
            && post_chain_output.is_finite()
            && routed_input.abs() <= threshold.amplitude()
            && post_chain_output.abs() <= threshold.amplitude())
    }

    fn refresh_active_slots(&mut self) {
        let (active_slot_indices, active_slot_count) = active_slots(&self.slot_params);
        self.active_slot_indices = active_slot_indices;
        self.active_slot_count = active_slot_count;
    }
}

fn active_slots(params: &[FxBusParams; BUS_SLOTS_PER_BUS]) -> ([usize; BUS_SLOTS_PER_BUS], usize) {
    let mut indices = [0; BUS_SLOTS_PER_BUS];
    let mut count = 0;
    for (index, param) in params.iter().enumerate() {
        if !matches!(param, FxBusParams::None) {
            indices[count] = index;
            count += 1;
        }
    }
    (indices, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::fx_params::FxKind;

    #[test]
    fn cost_table_is_exhaustive_and_worker_capacity_is_symbolic() {
        let kinds = [
            (FxKind::None, 0),
            (FxKind::Duck, 1),
            (FxKind::Distortion, 1),
            (FxKind::Bitcrusher, 1),
            (FxKind::Tremolo, 2),
            (FxKind::Delay, 2),
            (FxKind::Glitch, 2),
            (FxKind::AutoPan, 2),
            (FxKind::Saturator, 2),
            (FxKind::Vibrato, 3),
            (FxKind::Chorus, 3),
            (FxKind::Flanger, 3),
            (FxKind::Reverb, 3),
            (FxKind::Eq, 3),
            (FxKind::FilterLfo, BUS_CHAIN_SLOT_COST_UNITS),
            (FxKind::Wah, BUS_CHAIN_SLOT_COST_UNITS),
            (FxKind::Compressor, BUS_CHAIN_SLOT_COST_UNITS),
            (FxKind::Vinyl, BUS_CHAIN_SLOT_COST_UNITS),
        ];
        for (kind, cost) in kinds {
            assert_eq!(fx_kind_cost(kind), cost);
        }
    }

    #[test]
    fn exact_threshold_requires_exact_zero() {
        let mut owner = BusChainOwner::new(
            0,
            [FxBusParams::Tremolo {
                rate_hz: 1.0,
                depth: 0.0,
            }; BUS_SLOTS_PER_BUS],
            std::array::from_fn(|_| FxBusState::None),
            [1; BUS_SLOTS_PER_BUS],
        );
        owner.assigned_worker = Some(1);
        owner.observe(0.0, 0.0, BusIdleThreshold::Exact, 4_000);
        assert_eq!(owner.quiet_frames, 1);
        owner.observe(f32::EPSILON, 0.0, BusIdleThreshold::Exact, 4_000);
        assert_eq!(owner.quiet_frames, 0);
    }
}
