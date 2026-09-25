use super::pluck_string::{PluckRing, RING_LEN};
use super::runtime_state::Voice;
use super::types::{
    SYNTH_VOICE_LANE_CAPACITY, SYNTH_VOICE_PARTITION_LANE_CAPACITY, VOICE_PARTITION_COUNT,
};

pub(super) struct SynthVoicePartition {
    pub(super) parity: usize,
    pub(super) lanes: [Voice; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
    pub(super) rings: [Option<Box<PluckRing>>; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
    pub(super) render_lanes: [usize; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
    pub(super) render_lane_count: usize,
}

impl SynthVoicePartition {
    pub(super) fn new(parity: usize) -> Self {
        Self {
            parity,
            lanes: [Voice::off(); SYNTH_VOICE_PARTITION_LANE_CAPACITY],
            rings: std::array::from_fn(|_| Some(Box::new([0.0; RING_LEN]))),
            render_lanes: [0; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
            render_lane_count: 0,
        }
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn empty(parity: usize) -> Self {
        Self {
            parity,
            lanes: [Voice::off(); SYNTH_VOICE_PARTITION_LANE_CAPACITY],
            rings: std::array::from_fn(|_| None),
            render_lanes: [0; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
            render_lane_count: 0,
        }
    }

    pub(super) fn lane_and_ring_mut(
        &mut self,
        lane: usize,
    ) -> Option<(&mut Voice, &mut PluckRing)> {
        Some((
            self.lanes.get_mut(lane)?,
            self.rings.get_mut(lane)?.as_deref_mut()?,
        ))
    }

    pub(super) fn active_count(&self) -> usize {
        self.lanes.iter().filter(|voice| voice.active).count()
    }

    pub(super) fn rebuild_render_lanes(
        &mut self,
        lane_slots: &[Option<usize>; SYNTH_VOICE_LANE_CAPACITY],
    ) {
        let mut count = 0;
        for (global_lane, owner) in lane_slots.iter().enumerate() {
            if owner.is_some() && global_lane % VOICE_PARTITION_COUNT == self.parity {
                self.render_lanes[count] = global_lane / VOICE_PARTITION_COUNT;
                count += 1;
            }
        }
        self.render_lane_count = count;
    }
}
