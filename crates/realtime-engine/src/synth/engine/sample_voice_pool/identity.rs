use super::super::support::SampleVoice;
use super::SampleVoicePool;

impl SampleVoicePool {
    pub(in crate::synth::engine) fn deactivate_lane(&mut self, lane: usize) -> bool {
        let slot = self.lane_slots.get(lane).copied().flatten();
        if let Some(slot) = slot {
            self.remove_lane(slot, lane);
            self.lane_slots[lane] = None;
        }
        let Some(voice) = self.lane_mut(lane) else {
            return false;
        };
        voice.active = false;
        true
    }

    /// Starts a steal fade: the voice keeps its lane until it is silent, then frees it.
    pub(in crate::synth::engine) fn fade_out_lane(&mut self, lane: usize) -> bool {
        let Some(voice) = self.lane_mut(lane).filter(|voice| voice.active) else {
            return false;
        };
        voice.declick.begin_fade();
        true
    }

    /// Active voices a steal has not started fading out.
    pub(in crate::synth::engine) fn sounding_count_for_slot(&self, slot: usize) -> usize {
        self.slot_lanes(slot).map_or(0, |lanes| {
            lanes
                .iter()
                .filter(|lane| self.lane(**lane).is_some_and(is_sounding))
                .count()
        })
    }
}

fn is_sounding(voice: &SampleVoice) -> bool {
    voice.active && !voice.declick.is_fading()
}
