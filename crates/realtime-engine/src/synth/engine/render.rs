use super::inline_source_executor::SourceRenderOutput;
use super::render_profile;
use super::source_lane_renderer::{SampleSourceContext, SynthSourceContext};
use super::*;
use crate::simd::interleave_stereo;
use std::time::Instant;

impl SynthEngine {
    pub fn next_sample(&mut self) -> f32 {
        let (l, r) = self.next_stereo_sample();
        (l + r) * 0.5
    }

    pub fn next_stereo_sample(&mut self) -> (f32, f32) {
        if self.render_profile.enabled {
            return self.profiled_serial_frame_graph();
        }
        self.serial_frame_graph()
    }

    fn serial_frame_graph(&mut self) -> (f32, f32) {
        if !self.voice_pools_home() {
            return (0.0, 0.0);
        }
        let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];
        let sample_active = self.render_sample_voices(&mut slot_out);
        let preview_active = self.render_preview_sample_voices(&mut slot_out);
        let synth_active = self.render_synth_voices(&mut slot_out);
        self.finish_serial_frame(slot_out, sample_active, preview_active, synth_active)
    }

    fn finish_serial_frame(
        &mut self,
        slot_out: [f32; INSTRUMENT_SLOT_COUNT],
        sample_active: bool,
        preview_active: bool,
        synth_active: bool,
    ) -> (f32, f32) {
        let process_buses = self.should_process_fx_buses();
        if process_buses {
            self.prepare_bus_buffers();
        }
        let (mut left, mut right) = self.mix_instrument_slots(&slot_out);
        if process_buses {
            (left, right) = self.mix_fx_buses(&slot_out, left, right);
        }
        self.push_dry_history(left, right);
        let master_signal = self.signal_present(left, right)
            || synth_active
            || sample_active
            || preview_active
            || !self.momentary_fx.is_empty()
            || self.active_bus_activity_count > 0;
        let master_active = master_signal || self.master_activity_frames > 0;
        if master_active {
            (left, right) = self.apply_master_fx_slots(left, right);
            (left, right) =
                self.process_momentary_fx_target(MomentaryFxTarget::Global, left, right);
            self.master_activity_frames = if master_signal || self.signal_present(left, right) {
                self.fx_activity_hold_frames
            } else {
                self.master_activity_frames.saturating_sub(1)
            };
        }
        self.sample_clock = self.sample_clock.saturating_add(1);
        (
            (left * self.master_volume).clamp(-1.0, 1.0),
            (right * self.master_volume).clamp(-1.0, 1.0),
        )
    }

    fn block_slot_frame_graph(
        &mut self,
        frames: usize,
        left_out: &mut [f32],
        right_out: &mut [f32],
    ) {
        if !self.voice_pools_home() {
            left_out[..frames].fill(0.0);
            right_out[..frames].fill(0.0);
            return;
        }
        {
            let scratch = &mut self.block_slot_scratch;
            scratch.inline_source_executor.render_sample_sources(
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
        scratch.inline_source_executor.render_synth_sources(
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
        for frame in 0..frames {
            let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];
            let mut sample_active = false;
            let mut synth_active = false;
            for (slot, out) in slot_out.iter_mut().enumerate() {
                *out += self.block_slot_scratch.sample_slot_out[slot][frame];
                sample_active |= self.block_slot_scratch.sample_active[slot][frame];
            }
            let preview_active = self.render_preview_sample_voices(&mut slot_out);
            for (slot, out) in slot_out.iter_mut().enumerate() {
                *out += self.block_slot_scratch.synth_slot_out[slot][frame];
                synth_active |= self.block_slot_scratch.synth_active[slot][frame];
            }
            let (left, right) =
                self.finish_serial_frame(slot_out, sample_active, preview_active, synth_active);
            left_out[frame] = left;
            right_out[frame] = right;
        }
    }

    fn profiled_serial_frame_graph(&mut self) -> (f32, f32) {
        if !self.voice_pools_home() {
            return (0.0, 0.0);
        }
        let frame_start = Instant::now();
        let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];

        let start = Instant::now();
        let sample_active = self.render_sample_voices(&mut slot_out);
        self.render_profile.stage_ns[render_profile::PROFILE_SAMPLE_VOICES] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        let preview_active = self.render_preview_sample_voices(&mut slot_out);
        self.render_profile.stage_ns[render_profile::PROFILE_PREVIEW_SAMPLE_VOICES] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        let synth_active = self.render_synth_voices(&mut slot_out);
        self.render_profile.stage_ns[render_profile::PROFILE_SYNTH_VOICES] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        let process_buses = self.should_process_fx_buses();
        if process_buses {
            self.prepare_bus_buffers();
        }
        let (mut left, mut right) = self.mix_instrument_slots(&slot_out);
        self.render_profile.stage_ns[render_profile::PROFILE_PREPARE_MIX_SLOTS] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        if process_buses {
            (left, right) = self.mix_fx_buses(&slot_out, left, right);
        }
        self.render_profile.stage_ns[render_profile::PROFILE_FX_BUSES] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        self.push_dry_history(left, right);
        self.render_profile.stage_ns[render_profile::PROFILE_DRY_HISTORY] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        let master_signal = self.signal_present(left, right)
            || synth_active
            || sample_active
            || preview_active
            || !self.momentary_fx.is_empty()
            || self.active_bus_activity_count > 0;
        let master_active = master_signal || self.master_activity_frames > 0;
        if master_active {
            (left, right) = self.apply_master_fx_slots(left, right);
            (left, right) =
                self.process_momentary_fx_target(MomentaryFxTarget::Global, left, right);
            self.master_activity_frames = if master_signal || self.signal_present(left, right) {
                self.fx_activity_hold_frames
            } else {
                self.master_activity_frames.saturating_sub(1)
            };
        }
        self.render_profile.stage_ns[render_profile::PROFILE_MASTER_GLOBAL_FX] =
            start.elapsed().as_nanos() as u64;

        let start = Instant::now();
        self.sample_clock = self.sample_clock.saturating_add(1);
        let out = (
            (left * self.master_volume).clamp(-1.0, 1.0),
            (right * self.master_volume).clamp(-1.0, 1.0),
        );
        self.render_profile.stage_ns[render_profile::PROFILE_CLOCK_VOLUME_CLAMP] =
            start.elapsed().as_nanos() as u64;
        self.render_profile.frames_observed = self.render_profile.frames_observed.saturating_add(1);
        self.render_profile.last_frame_total_ns = frame_start.elapsed().as_nanos() as u64;
        out
    }

    pub fn render_interleaved_block(
        &mut self,
        frames: usize,
        left: &mut Vec<f32>,
        right: &mut Vec<f32>,
        out: &mut Vec<f32>,
    ) {
        left.resize(frames, 0.0);
        right.resize(frames, 0.0);
        out.resize(frames * 2, 0.0);
        if !self.render_profile.enabled && self.block_slot_scratch.prepare(frames) {
            self.block_slot_frame_graph(frames, left, right);
            interleave_stereo(left, right, out);
            return;
        }
        let block_start = self.render_profile.enabled.then(Instant::now);
        for frame in 0..frames {
            let (l, r) = self.next_stereo_sample();
            left[frame] = l;
            right[frame] = r;
        }
        let interleave_start = self.render_profile.enabled.then(Instant::now);
        interleave_stereo(left, right, out);
        if let Some(start) = interleave_start {
            self.render_profile.interleave_ns = start.elapsed().as_nanos() as u64;
            self.render_profile.blocks_observed =
                self.render_profile.blocks_observed.saturating_add(1);
            self.render_profile.last_block_frames = frames;
            self.render_profile.last_block_total_ns = block_start
                .map(|block_start| block_start.elapsed().as_nanos() as u64)
                .unwrap_or(self.render_profile.interleave_ns);
        }
    }
}
