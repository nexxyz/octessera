use super::super::source_lane_renderer::INVALID_INSTRUMENT_SLOT;
use super::SourceWorkerScratch;
use super::*;
use crate::synth::types::{
    INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
};
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static REDUCTION_LANE_COUNTS: Cell<[(usize, usize); 2]> =
        const { Cell::new([(0, 0); 2]) };
}

#[cfg(test)]
fn record_reduction_lane(index: usize, reduced: bool) {
    REDUCTION_LANE_COUNTS.with(|counts| {
        let mut value = counts.get();
        value[index].0 += 1;
        if reduced {
            value[index].1 += 1;
        }
        counts.set(value);
    });
}

impl SourceWorkerRuntime {
    pub(super) fn reduce_sources(
        &self,
        engine: &mut SynthEngine,
        scratch: [&SourceWorkerScratch; SOURCE_WORKER_COUNT],
        frames: usize,
    ) {
        #[cfg(test)]
        REDUCTION_LANE_COUNTS.with(|counts| counts.set([(0, 0); 2]));

        for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
            let parity = lane % SOURCE_WORKER_COUNT;
            let local_lane = lane / SOURCE_WORKER_COUNT;
            let source = scratch[parity];
            let slot = source.sample.slots[local_lane];
            let valid_slot =
                slot != INVALID_INSTRUMENT_SLOT && (slot as usize) < INSTRUMENT_SLOT_COUNT;
            #[cfg(test)]
            record_reduction_lane(0, valid_slot);
            if !valid_slot {
                continue;
            }
            let slot = slot as usize;
            for frame in 0..frames {
                engine.block_slot_scratch.sample_slot_out[slot][frame] +=
                    source.sample.samples[local_lane][frame];
                engine.block_slot_scratch.sample_active[slot][frame] |=
                    source.sample.active[local_lane][frame];
            }
        }

        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let parity = lane % SOURCE_WORKER_COUNT;
            let local_lane = lane / SOURCE_WORKER_COUNT;
            let source = scratch[parity];
            let slot = source.synth.slots[local_lane];
            let valid_slot =
                slot != INVALID_INSTRUMENT_SLOT && (slot as usize) < INSTRUMENT_SLOT_COUNT;
            #[cfg(test)]
            record_reduction_lane(1, valid_slot);
            if !valid_slot {
                continue;
            }
            let slot = slot as usize;
            for frame in 0..frames {
                engine.block_slot_scratch.synth_slot_out[slot][frame] +=
                    source.synth.samples[local_lane][frame];
                engine.block_slot_scratch.synth_active[slot][frame] |=
                    source.synth.active[local_lane][frame];
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn reduction_lane_counts_for_test(&self) -> [(usize, usize); 2] {
        REDUCTION_LANE_COUNTS.with(Cell::get)
    }
}

#[cfg(test)]
mod tests {
    use super::super::SourceWorkerScratch;
    use super::*;

    #[test]
    fn valid_slot_zero_is_reduced_and_invalid_slots_are_skipped() {
        let runtime = SourceWorkerRuntime::inline();
        let mut engine = SynthEngine::new(48_000);
        let mut first = SourceWorkerScratch::new();
        let second = SourceWorkerScratch::new();

        first.sample.slots[0] = 0;
        first.sample.samples[0][0] = 0.25;
        first.sample.active[0][0] = true;
        first.sample.samples[1][0] = 4.0;
        first.sample.active[1][0] = true;
        first.synth.slots[0] = 0;
        first.synth.samples[0][0] = -0.5;
        first.synth.active[0][0] = true;
        first.synth.samples[1][0] = 8.0;
        first.synth.active[1][0] = true;

        runtime.reduce_sources(&mut engine, [&first, &second], 1);

        assert_eq!(
            engine.block_slot_scratch.sample_slot_out[0][0].to_bits(),
            0.25_f32.to_bits()
        );
        assert_eq!(
            engine.block_slot_scratch.synth_slot_out[0][0].to_bits(),
            (-0.5_f32).to_bits()
        );
        assert!(engine.block_slot_scratch.sample_active[0][0]);
        assert!(engine.block_slot_scratch.synth_active[0][0]);
        assert_eq!(runtime.reduction_lane_counts_for_test(), [(64, 1), (64, 1)]);
    }
}
