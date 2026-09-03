use super::super::super::types::{INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY};
use super::SampleVoicePool;

impl SampleVoicePool {
    pub(in crate::synth::engine) fn assert_invariants(&self) {
        assert!(self.partitions_home());
        let mut ownership_counts = [0; SAMPLE_VOICE_LANE_CAPACITY];
        let mut canonical_ownership_counts = [0; SAMPLE_VOICE_LANE_CAPACITY];
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let count = self.slot_lane_counts[slot];
            assert!(count <= SAMPLE_VOICE_LANE_CAPACITY);
            for &lane in &self.slot_lanes[slot][..count] {
                assert!(lane < SAMPLE_VOICE_LANE_CAPACITY);
                ownership_counts[lane] += 1;
                assert_eq!(self.lane_slots[lane], Some(slot));
                let voice = self.lane(lane).expect("home partition lane");
                assert!(voice.active);
                let canonical_lane = voice.canonical_lane.expect("active canonical lane");
                assert!((canonical_lane as usize) < SAMPLE_VOICE_LANE_CAPACITY);
                canonical_ownership_counts[canonical_lane as usize] += 1;
                assert_eq!(voice.instrument_slot as usize, slot);
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
                    let voice = self.lane(lane).expect("home partition lane");
                    assert!(!voice.active);
                    assert!(voice.canonical_lane.is_none());
                }
            }
        }
        assert!(canonical_ownership_counts
            .into_iter()
            .all(|count| count <= 1));
    }
}
