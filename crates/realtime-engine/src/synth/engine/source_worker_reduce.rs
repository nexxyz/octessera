use super::SourceWorkerScratch;
use super::*;
use crate::synth::types::{
    INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
};

impl SourceWorkerRuntime {
    pub(super) fn reduce_sources(
        &self,
        engine: &mut SynthEngine,
        scratch: [&SourceWorkerScratch; SOURCE_WORKER_COUNT],
        frames: usize,
    ) {
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
                let parity = lane % SOURCE_WORKER_COUNT;
                let local_lane = lane / SOURCE_WORKER_COUNT;
                let source = scratch[parity];
                if source.sample.slots[local_lane] as usize != slot {
                    continue;
                }
                for frame in 0..frames {
                    engine.block_slot_scratch.sample_slot_out[slot][frame] +=
                        source.sample.samples[local_lane][frame];
                    engine.block_slot_scratch.sample_active[slot][frame] |=
                        source.sample.active[local_lane][frame];
                }
            }
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
                let parity = lane % SOURCE_WORKER_COUNT;
                let local_lane = lane / SOURCE_WORKER_COUNT;
                let source = scratch[parity];
                if source.synth.slots[local_lane] as usize != slot {
                    continue;
                }
                for frame in 0..frames {
                    engine.block_slot_scratch.synth_slot_out[slot][frame] +=
                        source.synth.samples[local_lane][frame];
                    engine.block_slot_scratch.synth_active[slot][frame] |=
                        source.synth.active[local_lane][frame];
                }
            }
        }
    }
}
