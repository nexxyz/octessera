use super::super::types::{INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY};
use super::support::SampleVoice;

pub(super) struct SampleVoicePool {
    lanes: [SampleVoice; SAMPLE_VOICE_LANE_CAPACITY],
    slot_lanes: [[usize; SAMPLE_VOICE_LANE_CAPACITY]; INSTRUMENT_SLOT_COUNT],
    slot_lane_counts: [usize; INSTRUMENT_SLOT_COUNT],
    lane_slots: [Option<usize>; SAMPLE_VOICE_LANE_CAPACITY],
}

impl SampleVoicePool {
    pub(super) fn new() -> Self {
        Self {
            lanes: [SampleVoice::off(); SAMPLE_VOICE_LANE_CAPACITY],
            slot_lanes: [[0; SAMPLE_VOICE_LANE_CAPACITY]; INSTRUMENT_SLOT_COUNT],
            slot_lane_counts: [0; INSTRUMENT_SLOT_COUNT],
            lane_slots: [None; SAMPLE_VOICE_LANE_CAPACITY],
        }
    }

    pub(super) fn lane(&self, lane: usize) -> &SampleVoice {
        &self.lanes[lane]
    }

    pub(super) fn lane_mut(&mut self, lane: usize) -> &mut SampleVoice {
        &mut self.lanes[lane]
    }

    pub(super) fn active_total(&self) -> usize {
        self.lanes.iter().filter(|voice| voice.active).count()
    }

    pub(super) fn active_count_for_slot(&self, slot: usize) -> usize {
        self.slot_lanes[slot][..self.slot_lane_counts[slot]]
            .iter()
            .filter(|lane| self.lanes[**lane].active)
            .count()
    }

    pub(super) fn first_inactive_lane(&self) -> Option<usize> {
        self.lanes.iter().position(|voice| !voice.active)
    }

    pub(super) fn first_active_lane_for_slot(&self, slot: usize) -> Option<usize> {
        self.slot_lanes[slot][..self.slot_lane_counts[slot]]
            .iter()
            .copied()
            .find(|lane| self.lanes[*lane].active)
    }

    pub(super) fn first_active_lane_global(&self) -> Option<(usize, usize)> {
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            if let Some(lane) = self.first_active_lane_for_slot(slot) {
                return Some((slot, lane));
            }
        }
        None
    }

    pub(super) fn slot_lanes(&self, slot: usize) -> &[usize] {
        &self.slot_lanes[slot][..self.slot_lane_counts[slot]]
    }

    pub(super) fn compact_slot_lanes(&mut self, slot: usize) {
        let mut write = 0;
        let count = self.slot_lane_counts[slot];
        for read in 0..count {
            let lane = self.slot_lanes[slot][read];
            if self.lanes[lane].active {
                self.slot_lanes[slot][write] = lane;
                write += 1;
            } else {
                self.lane_slots[lane] = None;
            }
        }
        self.slot_lane_counts[slot] = write;
    }

    pub(super) fn clear_all(&mut self) {
        for voice in &mut self.lanes {
            voice.active = false;
        }
        self.slot_lane_counts = [0; INSTRUMENT_SLOT_COUNT];
        self.lane_slots = [None; SAMPLE_VOICE_LANE_CAPACITY];
    }

    pub(super) fn clear_slot(&mut self, slot: usize) {
        let count = self.slot_lane_counts[slot];
        for index in 0..count {
            let lane = self.slot_lanes[slot][index];
            self.lanes[lane].active = false;
            self.lane_slots[lane] = None;
        }
        self.slot_lane_counts[slot] = 0;
    }

    pub(super) fn assign_lane(&mut self, lane: usize, slot: usize) {
        if self.lane_slots[lane] == Some(slot) {
            return;
        }
        if let Some(previous_slot) = self.lane_slots[lane] {
            self.remove_lane(previous_slot, lane);
        }
        let count = self.slot_lane_counts[slot];
        let mut insert = count;
        while insert > 0 && self.slot_lanes[slot][insert - 1] > lane {
            self.slot_lanes[slot][insert] = self.slot_lanes[slot][insert - 1];
            insert -= 1;
        }
        self.slot_lanes[slot][insert] = lane;
        self.slot_lane_counts[slot] = count + 1;
        self.lane_slots[lane] = Some(slot);
    }

    fn remove_lane(&mut self, slot: usize, lane: usize) {
        let count = self.slot_lane_counts[slot];
        let Some(index) = self.slot_lanes[slot][..count]
            .iter()
            .position(|candidate| *candidate == lane)
        else {
            return;
        };
        for shift in index..count - 1 {
            self.slot_lanes[slot][shift] = self.slot_lanes[slot][shift + 1];
        }
        self.slot_lane_counts[slot] = count - 1;
    }

    #[cfg(test)]
    pub(super) fn assert_invariants(&self) {
        let mut ownership_counts = [0; SAMPLE_VOICE_LANE_CAPACITY];
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let count = self.slot_lane_counts[slot];
            assert!(count <= SAMPLE_VOICE_LANE_CAPACITY);
            for &lane in &self.slot_lanes[slot][..count] {
                assert!(lane < SAMPLE_VOICE_LANE_CAPACITY);
                ownership_counts[lane] += 1;
                assert_eq!(self.lane_slots[lane], Some(slot));
                assert!(self.lanes[lane].active);
                assert_eq!(self.lanes[lane].instrument_slot as usize, slot);
            }
        }
        for (lane, &ownership_count) in ownership_counts.iter().enumerate() {
            match self.lane_slots[lane] {
                Some(slot) => {
                    assert!(slot < INSTRUMENT_SLOT_COUNT);
                    assert_eq!(ownership_count, 1);
                }
                None => {
                    assert_eq!(ownership_count, 0);
                    assert!(!self.lanes[lane].active);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LANES: usize = 9;
    const _: () = assert!(SAMPLE_VOICE_LANE_CAPACITY >= TEST_LANES);

    #[test]
    fn one_slot_can_assign_and_iterate_more_than_eight_lanes() {
        let mut pool = SampleVoicePool::new();
        for lane in 0..TEST_LANES {
            pool.assign_lane(lane, 0);
            pool.lane_mut(lane).active = true;
        }

        assert_eq!(pool.slot_lanes(0), (0..TEST_LANES).collect::<Vec<_>>());
        assert_eq!(pool.active_count_for_slot(0), TEST_LANES);
        pool.assert_invariants();
    }

    #[test]
    fn repeated_assignment_compaction_clearing_and_reuse_preserve_invariants() {
        let mut pool = SampleVoicePool::new();
        for round in 0..32 {
            for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
                let slot = (lane + round) % INSTRUMENT_SLOT_COUNT;
                pool.assign_lane(lane, slot);
                let voice = pool.lane_mut(lane);
                voice.instrument_slot = slot as u8;
                voice.active = true;
            }
            for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
                if (lane + round) % 3 == 0 {
                    pool.lane_mut(lane).active = false;
                }
            }
            for slot in 0..INSTRUMENT_SLOT_COUNT {
                pool.compact_slot_lanes(slot);
            }
            pool.assert_invariants();
        }

        pool.clear_slot(0);
        pool.assert_invariants();
        pool.clear_all();
        pool.assert_invariants();

        for lane in 0..TEST_LANES {
            pool.assign_lane(lane, 0);
            let voice = pool.lane_mut(lane);
            voice.instrument_slot = 0;
            voice.active = true;
        }
        pool.assert_invariants();
    }
}
