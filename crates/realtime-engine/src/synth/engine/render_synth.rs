use super::source_lane_renderer::{render_synth_voice_frame, SynthVoiceFrameContext};
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
        if !self.synth_voice_pool.compact_slot_lanes(slot_idx) {
            return SlotFrameOutput::default();
        }
        let mut lane_indices = [0; SYNTH_VOICE_LANE_CAPACITY];
        let Some(lanes) = self.synth_voice_pool.slot_lanes(slot_idx) else {
            return SlotFrameOutput::default();
        };
        let lane_count = lanes.len();
        lane_indices[..lane_count].copy_from_slice(lanes);
        let context = SynthVoiceFrameContext {
            sample_rate: self.sample_rate,
            config: self.instruments[slot_idx],
            render_config: self.synth_render_configs[slot_idx],
            revision: self.synth_render_revisions[slot_idx],
            mods: self.mods[slot_idx],
        };
        let mut sample = 0.0;
        let mut active = false;
        for lane in lane_indices.into_iter().take(lane_count) {
            let Some(voice) = self.synth_voice_pool.lane_mut(lane) else {
                return SlotFrameOutput::default();
            };
            if let Some(rendered) =
                render_synth_voice_frame(voice, slot_idx, frame_sample_clock, context)
            {
                sample += rendered;
                active = true;
            }
        }
        self.synth_voice_pool.compact_slot_lanes(slot_idx);
        let rendered = SlotFrameOutput { sample, active };
        self.active_synth_slots[slot_idx] = rendered.active;
        rendered
    }
}
