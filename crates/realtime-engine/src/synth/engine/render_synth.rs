use super::render_synth_parallel::render_synth_slot_pool_frame;
use super::*;

impl SynthEngine {
    pub(super) fn render_synth_voices(
        &mut self,
        slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT],
    ) -> bool {
        let mut active = false;
        for (slot_idx, out) in slot_out.iter_mut().enumerate().take(INSTRUMENT_SLOT_COUNT) {
            let rendered = self.render_synth_slot(slot_idx);
            *out += rendered.sample;
            active |= rendered.active;
        }
        active
    }

    pub(super) fn render_synth_slot(&mut self, slot_idx: usize) -> SlotFrameOutput {
        self.render_synth_slot_at(slot_idx, self.sample_clock)
    }

    pub(super) fn render_synth_slot_at(
        &mut self,
        slot_idx: usize,
        frame_sample_clock: u64,
    ) -> SlotFrameOutput {
        if !self.active_synth_slots[slot_idx] {
            return SlotFrameOutput::default();
        }
        let rendered = render_synth_slot_pool_frame(
            &mut self.voices[slot_idx],
            slot_idx,
            frame_sample_clock,
            render_synth_parallel::SynthSlotFrameContext {
                sample_rate: self.sample_rate,
                config: self.instruments[slot_idx],
                render_config: &self.synth_render_configs[slot_idx],
                revision: self.synth_render_revisions[slot_idx],
                mods: self.mods[slot_idx],
            },
        );
        self.active_synth_slots[slot_idx] = rendered.active;
        rendered
    }
}
