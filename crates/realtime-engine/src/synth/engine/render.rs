use super::inline_source_executor::SourceRenderOutput;
use super::render_voice::synth_voice_render::SynthSourceContext;
use super::sample_voice_renderer::SampleSourceContext;
use super::*;
use crate::simd::interleave_stereo;

impl SynthEngine {
    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(super) fn synth_source_context(&self) -> SynthSourceContext {
        SynthSourceContext {
            sample_rate: self.sample_rate,
            configs: self.instruments,
            render_configs: self.synth_render_configs,
            revisions: self.synth_render_revisions,
            mods: self.mods,
        }
    }

    pub fn next_sample(&mut self) -> f32 {
        let (l, r) = self.next_stereo_sample();
        (l + r) * 0.5
    }

    pub fn next_stereo_sample(&mut self) -> (f32, f32) {
        self.serial_frame_graph()
    }

    fn serial_frame_graph(&mut self) -> (f32, f32) {
        if !self.voice_pools_home() {
            return (0.0, 0.0);
        }
        let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];
        let sample_active = self.render_sample_voices(&mut slot_out);
        let (preview, preview_active) = self.render_preview_sample_voices();
        let synth_active = self.render_synth_voices(&mut slot_out);
        let output = self.finish_serial_frame::<false>(
            slot_out,
            sample_active,
            preview,
            preview_active,
            synth_active,
            0,
        );
        self.recover_from_nonfinite_output();
        output
    }

    /// One frame's mix down to the master output; with `TAP` it also keeps `tapped_bus`'s
    /// chain output in `tapped_bus_sample`.
    pub(super) fn finish_serial_frame<const TAP: bool>(
        &mut self,
        slot_out: [f32; INSTRUMENT_SLOT_COUNT],
        sample_active: bool,
        preview: f32,
        preview_active: bool,
        synth_active: bool,
        tapped_bus: usize,
    ) -> (f32, f32) {
        let process_buses = self.should_process_fx_buses();
        if process_buses {
            self.prepare_bus_buffers();
        }
        let (mut left, mut right) = self.mix_instrument_slots(&slot_out);
        if TAP {
            self.tapped_bus_sample = 0.0;
        }
        if process_buses {
            (left, right) = self.mix_fx_buses::<TAP>(&slot_out, left, right, tapped_bus);
        }
        left += preview;
        right += preview;
        let sources_active =
            synth_active || sample_active || preview_active || self.active_bus_activity_count > 0;
        let (left, right, clipped) = self.finish_master_frame(left, right, sources_active);
        self.output_clipped |= clipped;
        (left, right)
    }

    /// The master stage of one frame: master FX and Global momentary FX while the master is
    /// active, the activity hold, master volume and the clamp; also returns whether it clipped.
    /// A non-finite frame comes out silent, counts as clipped and marks the FX state for reset.
    #[inline]
    fn finish_master_frame(
        &mut self,
        mut left: f32,
        mut right: f32,
        sources_active: bool,
    ) -> (f32, f32, bool) {
        let master_signal =
            self.signal_present(left, right) || sources_active || !self.momentary_fx.is_empty();
        if master_signal || self.master_activity_frames > 0 {
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
        let left = left * self.master_volume;
        let right = right * self.master_volume;
        if !(left.is_finite() && right.is_finite()) {
            self.master_nonfinite = true;
            return (0.0, 0.0, true);
        }
        let clipped = left.abs().max(right.abs()) > 1.0;
        (left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0), clipped)
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
            let executor = scratch
                .inline_source_executor
                .as_mut()
                .expect("inline source executor");
            assert!(executor.prepare(frames));
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
        let signal = self.signal_tap.selected();
        if let Some(SignalSource::Instrument(slot)) = signal {
            let scratch = &self.block_slot_scratch;
            let block = scratch.sample_slot_out[slot][..frames]
                .iter()
                .zip(&scratch.synth_slot_out[slot][..frames])
                .map(|(sample, synth)| sample + synth);
            self.signal_tap.publish_mono(block, self.sample_rate);
        }
        match signal {
            Some(SignalSource::Bus(bus)) => {
                self.finish_block_slot_frame_graph::<true>(frames, left_out, right_out, bus);
                let block = self.block_slot_scratch.signal[..frames].iter().copied();
                self.signal_tap.publish_mono(block, self.sample_rate);
            }
            _ => self.finish_block_slot_frame_graph::<false>(frames, left_out, right_out, 0),
        }
    }

    fn finish_block_slot_frame_graph<const TAP: bool>(
        &mut self,
        frames: usize,
        left_out: &mut [f32],
        right_out: &mut [f32],
        tapped_bus: usize,
    ) {
        for frame in 0..frames {
            let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];
            let mut sample_active = false;
            let mut synth_active = false;
            for (slot, out) in slot_out.iter_mut().enumerate() {
                *out += self.block_slot_scratch.sample_slot_out[slot][frame];
                sample_active |= self.block_slot_scratch.sample_active[slot][frame];
            }
            let (preview, preview_active) = self.render_preview_sample_voices();
            for (slot, out) in slot_out.iter_mut().enumerate() {
                *out += self.block_slot_scratch.synth_slot_out[slot][frame];
                synth_active |= self.block_slot_scratch.synth_active[slot][frame];
            }
            let (left, right) = self.finish_serial_frame::<TAP>(
                slot_out,
                sample_active,
                preview,
                preview_active,
                synth_active,
                tapped_bus,
            );
            if TAP {
                self.block_slot_scratch.signal[frame] = self.tapped_bus_sample;
            }
            left_out[frame] = left;
            right_out[frame] = right;
        }
        self.recover_from_nonfinite_output();
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn finish_persistent_block(
        &mut self,
        frames: usize,
        left: &mut [f32],
        right: &mut [f32],
    ) {
        let mut clipped = false;
        for frame in 0..frames {
            let sources_active = self.block_slot_scratch.source_active[frame]
                || self.block_slot_scratch.bus_active[frame];
            let frame_clipped;
            (left[frame], right[frame], frame_clipped) =
                self.finish_master_frame(left[frame], right[frame], sources_active);
            clipped |= frame_clipped;
        }
        self.output_clipped |= clipped;
        self.recover_from_nonfinite_output();
    }

    #[inline]
    fn recover_from_nonfinite_output(&mut self) {
        if std::mem::take(&mut self.master_nonfinite) {
            self.silence_fx_state();
        }
    }

    /// Clears the master and bus FX history a non-finite frame may have left behind, ends the
    /// voices that went non-finite and retires the Play FX; the routing-tree workers do the same
    /// for what they hold when they see the new generation.
    #[cold]
    fn silence_fx_state(&mut self) {
        self.synth_voice_pool.end_poisoned_voices();
        self.sample_voice_pool.end_poisoned_voices();
        super::retired_state::retire_momentary_states(
            &mut self.momentary_fx,
            &mut self.pending_render_retired.displaced_momentary_fx,
        );
        for state in &mut self.master_slot_state {
            state.silence();
        }
        for chain in &mut self.bus_chains {
            chain.silence();
        }
        for spread in &mut self.bus_output_spread_state {
            spread.silence();
        }
        #[cfg(feature = "routing-tree-executor")]
        {
            self.fx_silence_generation = self.fx_silence_generation.wrapping_add(1);
        }
    }

    /// Whether the output went over full scale before the master clamp since the last call.
    pub fn take_output_clipped(&mut self) -> bool {
        std::mem::take(&mut self.output_clipped)
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
        if self.block_slot_scratch.prepare_output(frames) {
            self.block_slot_frame_graph(frames, left, right);
            interleave_stereo(left, right, out);
            return;
        }
        for frame in 0..frames {
            let (l, r) = self.next_stereo_sample();
            left[frame] = l;
            right[frame] = r;
        }
        interleave_stereo(left, right, out);
    }
}
