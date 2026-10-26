use super::super::runtime_state::Voice;
use super::super::types::{LogicalLaneId, SYNTH_VOICE_LANE_CAPACITY};
use super::SynthVoicePool;

impl SynthVoicePool {
    pub(in crate::synth) fn canonical_lane(&self, lane: usize) -> Option<LogicalLaneId> {
        self.lane(lane)?.canonical_lane
    }

    pub(in crate::synth) fn first_free_canonical_lane(&self) -> Option<LogicalLaneId> {
        if !self.partitions_home() {
            return None;
        }
        let mut used = [false; SYNTH_VOICE_LANE_CAPACITY];
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let voice = self.lane(lane)?;
            if let Some(canonical_lane) = voice.canonical_lane.filter(|_| voice.active) {
                let canonical_lane = canonical_lane as usize;
                if canonical_lane >= SYNTH_VOICE_LANE_CAPACITY {
                    return None;
                }
                used[canonical_lane] = true;
            }
        }
        used.iter()
            .position(|used| !*used)
            .map(|lane| lane as LogicalLaneId)
    }

    #[cfg(test)]
    pub(in crate::synth) fn deactivate_lane(&mut self, lane: usize) -> bool {
        let slot = self.lane_slots.get(lane).copied().flatten();
        if let Some(slot) = slot {
            self.remove_lane(slot, lane);
            self.lane_slots[lane] = None;
        }
        let Some(voice) = self.lane_mut(lane) else {
            return false;
        };
        voice.active = false;
        voice.canonical_lane = None;
        true
    }

    /// Starts a steal fade: the voice keeps its lane until it is silent, then frees it.
    pub(in crate::synth) fn fade_out_lane(&mut self, lane: usize) -> bool {
        let Some(voice) = self.lane_mut(lane).filter(|voice| voice.active) else {
            return false;
        };
        voice.declick.begin_fade();
        true
    }

    /// Active voices a steal has not started fading out.
    pub(in crate::synth) fn sounding_count_for_slot(&self, slot: usize) -> usize {
        self.slot_lanes(slot).map_or(0, |lanes| {
            lanes
                .iter()
                .filter(|lane| self.lane(**lane).is_some_and(is_sounding))
                .count()
        })
    }
}

fn is_sounding(voice: &Voice) -> bool {
    voice.active && !voice.declick.is_fading()
}
