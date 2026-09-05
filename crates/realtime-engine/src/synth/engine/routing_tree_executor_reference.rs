use super::super::super::types::{MomentaryFxTarget, BUS_COUNT, INSTRUMENT_SLOT_COUNT};
use super::super::bus_chain_owner::BusChainOwner;
use super::super::inline_source_executor::SourceRenderOutput;
use super::super::render_plan::RenderPlanRoute;
use super::super::routing_tree_plan::RoutingTreePlan;
use super::super::routing_tree_validation::valid_render_plan;
use super::super::source_lane_renderer::{SampleSourceContext, SynthSourceContext};
use super::super::{SynthEngine, BLOCK_SLOT_SCRATCH_FRAMES};

impl SynthEngine {
    pub(in crate::synth::engine) fn render_routing_tree_block_for_test(
        &mut self,
        frames: usize,
        left: &mut [f32],
        right: &mut [f32],
    ) -> bool {
        let plan = RoutingTreePlan::from_render_plan(&self.render_plan);
        self.render_routing_tree_block_with_plan(plan, frames, left, right)
    }

    pub(in crate::synth::engine) fn render_routing_tree_block_with_plan_for_test(
        &mut self,
        plan: RoutingTreePlan,
        frames: usize,
        left: &mut [f32],
        right: &mut [f32],
    ) -> bool {
        self.render_routing_tree_block_with_plan(plan, frames, left, right)
    }

    fn render_routing_tree_block_with_plan(
        &mut self,
        plan: RoutingTreePlan,
        frames: usize,
        left: &mut [f32],
        right: &mut [f32],
    ) -> bool {
        if left.len() < frames || right.len() < frames || !valid_render_plan(self, &plan) {
            left.get_mut(..frames).unwrap_or(&mut []).fill(0.0);
            right.get_mut(..frames).unwrap_or(&mut []).fill(0.0);
            return false;
        }
        let Some(synth_counts) = super::active_counts_by_slot(&self.synth_voice_pool) else {
            left[..frames].fill(0.0);
            right[..frames].fill(0.0);
            return false;
        };
        let Some(sample_counts) = super::active_sample_counts_by_slot(&self.sample_voice_pool)
        else {
            left[..frames].fill(0.0);
            right[..frames].fill(0.0);
            return false;
        };
        let instrument_kinds = std::array::from_fn(|slot| self.slot_kind[slot]);
        let bus_costs = std::array::from_fn(|bus| {
            self.bus_chains
                .get(bus)
                .map(BusChainOwner::cost_units)
                .unwrap_or(0)
        });
        if !self.routing_tree_scratch.prepare(frames)
            || !self.routing_tree_scratch.assign_workers(
                plan,
                instrument_kinds,
                synth_counts,
                sample_counts,
                bus_costs,
                self.bus_chains.len(),
            )
        {
            left[..frames].fill(0.0);
            right[..frames].fill(0.0);
            return false;
        }
        if !self.block_slot_scratch.prepare_output(frames)
            || !self.render_block_sources_for_test(frames)
        {
            left[..frames].fill(0.0);
            right[..frames].fill(0.0);
            return false;
        }
        for frame in 0..frames {
            let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];
            let mut sample_active = false;
            let mut synth_active = false;
            for (slot, output) in slot_out.iter_mut().enumerate() {
                *output = self.block_slot_scratch.sample_slot_out[slot][frame];
                sample_active |= self.block_slot_scratch.sample_active[slot][frame];
            }
            let preview_active = self.render_preview_sample_voices(&mut slot_out);
            for (slot, output) in slot_out.iter_mut().enumerate() {
                *output += self.block_slot_scratch.synth_slot_out[slot][frame];
                synth_active |= self.block_slot_scratch.synth_active[slot][frame];
            }
            self.block_slot_scratch.source_active[frame] =
                sample_active || preview_active || synth_active;
            self.stage_routing_tree_frame(frame, &slot_out);
            if !self.process_routing_tree_buses(frame, &slot_out) {
                left[..frames].fill(0.0);
                right[..frames].fill(0.0);
                return false;
            }
            let (mut frame_left, mut frame_right) = {
                let scratch = &self.routing_tree_scratch;
                (
                    scratch.worker_left[0][frame] + scratch.worker_left[1][frame],
                    scratch.worker_right[0][frame] + scratch.worker_right[1][frame],
                )
            };
            self.push_dry_history(frame_left, frame_right);
            let master_signal = self.signal_present(frame_left, frame_right)
                || sample_active
                || synth_active
                || preview_active
                || !self.momentary_fx.is_empty()
                || self.active_bus_activity_count > 0;
            let master_active = master_signal || self.master_activity_frames > 0;
            if master_active {
                (frame_left, frame_right) = self.apply_master_fx_slots(frame_left, frame_right);
                (frame_left, frame_right) = self.process_momentary_fx_target(
                    MomentaryFxTarget::Global,
                    frame_left,
                    frame_right,
                );
                self.master_activity_frames =
                    if master_signal || self.signal_present(frame_left, frame_right) {
                        self.fx_activity_hold_frames
                    } else {
                        self.master_activity_frames.saturating_sub(1)
                    };
            }
            self.sample_clock = self.sample_clock.saturating_add(1);
            left[frame] = (frame_left * self.master_volume).clamp(-1.0, 1.0);
            right[frame] = (frame_right * self.master_volume).clamp(-1.0, 1.0);
        }
        true
    }

    pub(in crate::synth::engine) fn render_block_sources_for_test(
        &mut self,
        frames: usize,
    ) -> bool {
        if !self.voice_pools_home() || frames > BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        {
            let scratch = &mut self.block_slot_scratch;
            let executor = scratch
                .inline_source_executor
                .as_mut()
                .expect("inline source executor");
            if !executor.prepare(frames) {
                return false;
            }
            executor.render_sample_sources(
                frames,
                &mut self.sample_voice_pool,
                SampleSourceContext {
                    sample_rate: self.sample_rate,
                },
                SourceRenderOutput {
                    slot_out: &mut scratch.sample_slot_out,
                    slot_active: &mut scratch.sample_active,
                    active_slots: &mut self.active_sample_slots,
                },
            );
        }
        let scratch = &mut self.block_slot_scratch;
        let executor = scratch
            .inline_source_executor
            .as_mut()
            .expect("inline source executor");
        executor.render_synth_sources(
            frames,
            self.sample_clock,
            &mut self.synth_voice_pool,
            SynthSourceContext {
                sample_rate: self.sample_rate,
                configs: self.instruments,
                render_configs: self.synth_render_configs,
                revisions: self.synth_render_revisions,
                mods: self.mods,
            },
            SourceRenderOutput {
                slot_out: &mut scratch.synth_slot_out,
                slot_active: &mut scratch.synth_active,
                active_slots: &mut self.active_synth_slots,
            },
        );
        true
    }

    fn stage_routing_tree_frame(&mut self, frame: usize, slot_out: &[f32; INSTRUMENT_SLOT_COUNT]) {
        let process_momentary = !self.momentary_fx.is_empty();
        for (slot, raw_sample) in slot_out.iter().enumerate() {
            let mut sample = *raw_sample * self.slot_volume[slot];
            if process_momentary {
                let (left, right) = self.process_momentary_fx_target(
                    MomentaryFxTarget::Instrument { index: slot },
                    sample,
                    sample,
                );
                sample = (left + right) * 0.5;
            }
            let Some(worker) = self.routing_tree_scratch.worker_for_slot(slot) else {
                continue;
            };
            match self.render_plan.instrument_slots[slot].route {
                RenderPlanRoute::Direct => {
                    let (pan_left, pan_right) = self.slot_pan_gains[slot];
                    self.routing_tree_scratch.worker_left[worker][frame] += sample * pan_left;
                    self.routing_tree_scratch.worker_right[worker][frame] += sample * pan_right;
                }
                RenderPlanRoute::Bus(bus) => {
                    self.routing_tree_scratch.bus_input[bus][frame] += sample;
                }
            }
        }
    }

    fn process_routing_tree_buses(
        &mut self,
        frame: usize,
        raw_slot_out: &[f32; INSTRUMENT_SLOT_COUNT],
    ) -> bool {
        let bus_input_snapshot: [f32; BUS_COUNT] =
            std::array::from_fn(|bus| self.routing_tree_scratch.bus_input[bus][frame]);
        for bus in 0..self.bus_chains.len() {
            let input = self.routing_tree_scratch.bus_input[bus][frame];
            let bus_active = self.signal_present_mono(input) || self.bus_chains[bus].is_active();
            if !bus_active {
                self.observe_bus_chain(bus, input, 0.0);
                continue;
            }
            let output = self.bus_chains[bus].process(
                input,
                raw_slot_out,
                &bus_input_snapshot,
                self.sample_rate,
            );
            self.observe_bus_chain(bus, input, output.mono);
            let input_present = self.signal_present_mono(input);
            let output_present = self.signal_present_mono(output.mono);
            self.bus_chains[bus].observe_render_hold(
                input_present,
                output_present,
                self.fx_activity_hold_frames,
            );
            let mut output = output;
            if !self.momentary_fx.is_empty() {
                let (left, right) = self.process_momentary_fx_target(
                    MomentaryFxTarget::FxBus { index: bus },
                    output.mono,
                    output.mono,
                );
                output.mono = (left + right) * 0.5;
            }
            let (left, right) = self.fx_bus_stereo_output(bus, output);
            let Some(worker) = self.routing_tree_scratch.worker_for_bus(bus) else {
                return false;
            };
            self.routing_tree_scratch.worker_left[worker][frame] += left;
            self.routing_tree_scratch.worker_right[worker][frame] += right;
        }
        self.active_bus_activity_count = self
            .bus_chains
            .iter()
            .filter(|chain| chain.is_active())
            .count();
        self.block_slot_scratch.bus_active[frame] = self.active_bus_activity_count > 0;
        true
    }
}
