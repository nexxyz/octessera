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
}
