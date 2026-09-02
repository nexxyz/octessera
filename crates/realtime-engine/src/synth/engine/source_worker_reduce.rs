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
    static PREFIX_ACTIVITY_APPLIES: Cell<[usize; 2]> = const { Cell::new([0; 2]) };
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

#[cfg(test)]
fn record_prefix_activity_apply(index: usize) {
    PREFIX_ACTIVITY_APPLIES.with(|applies| {
        let mut value = applies.get();
        value[index] += 1;
        applies.set(value);
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
        #[cfg(test)]
        PREFIX_ACTIVITY_APPLIES.with(|applies| applies.set([0; 2]));

        let mut sample_prefixes = [0; INSTRUMENT_SLOT_COUNT];
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
            sample_prefixes[slot] =
                sample_prefixes[slot].max(source.sample.rendered_frames[local_lane]);
            for frame in 0..frames {
                engine.block_slot_scratch.sample_slot_out[slot][frame] +=
                    source.sample.samples[local_lane][frame];
            }
        }
        for (slot, prefix) in sample_prefixes.into_iter().enumerate() {
            engine.block_slot_scratch.sample_active[slot][..prefix].fill(true);
            engine.block_slot_scratch.sample_active[slot][prefix..frames].fill(false);
        }
        #[cfg(test)]
        for _ in 0..INSTRUMENT_SLOT_COUNT {
            record_prefix_activity_apply(0);
        }

        let mut synth_prefixes = [0; INSTRUMENT_SLOT_COUNT];
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
            synth_prefixes[slot] =
                synth_prefixes[slot].max(source.synth.rendered_frames[local_lane]);
            for frame in 0..frames {
                engine.block_slot_scratch.synth_slot_out[slot][frame] +=
                    source.synth.samples[local_lane][frame];
            }
        }
        for (slot, prefix) in synth_prefixes.into_iter().enumerate() {
            engine.block_slot_scratch.synth_active[slot][..prefix].fill(true);
            engine.block_slot_scratch.synth_active[slot][prefix..frames].fill(false);
        }
        #[cfg(test)]
        for _ in 0..INSTRUMENT_SLOT_COUNT {
            record_prefix_activity_apply(1);
        }
    }

    #[cfg(test)]
    pub(crate) fn reduction_lane_counts_for_test(&self) -> [(usize, usize); 2] {
        REDUCTION_LANE_COUNTS.with(Cell::get)
    }

    #[cfg(test)]
    pub(crate) fn prefix_activity_applies_for_test(&self) -> [usize; 2] {
        PREFIX_ACTIVITY_APPLIES.with(Cell::get)
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
        first.sample.rendered_frames[0] = 1;
        first.sample.samples[1][0] = 4.0;
        first.sample.rendered_frames[1] = 1;
        first.synth.slots[0] = 0;
        first.synth.samples[0][0] = -0.5;
        first.synth.rendered_frames[0] = 1;
        first.synth.samples[1][0] = 8.0;
        first.synth.rendered_frames[1] = 1;

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
        assert_eq!(
            runtime.prefix_activity_applies_for_test(),
            [INSTRUMENT_SLOT_COUNT, INSTRUMENT_SLOT_COUNT]
        );
    }

    #[test]
    fn rendered_prefix_reduction_matches_the_boolean_reference_in_lane_order() {
        for frames in [32, 64, 128, 256, 2048] {
            for varied_prefixes in [false, true] {
                let scratch = reduction_scratch(frames, varied_prefixes);
                let expected_sample = boolean_reference(&scratch, frames, false);
                let expected_synth = boolean_reference(&scratch, frames, true);
                let runtime = SourceWorkerRuntime::inline();
                let mut engine = SynthEngine::new(48_000);
                engine.active_sample_slots[0] = true;
                engine.active_synth_slots[1] = true;
                let previous_sample_slots = engine.active_sample_slots;
                let previous_synth_slots = engine.active_synth_slots;

                runtime.reduce_sources(&mut engine, [&scratch[0], &scratch[1]], frames);

                assert_reduced_matches(
                    &engine.block_slot_scratch.sample_slot_out,
                    &engine.block_slot_scratch.sample_active,
                    &expected_sample,
                    frames,
                );
                assert_reduced_matches(
                    &engine.block_slot_scratch.synth_slot_out,
                    &engine.block_slot_scratch.synth_active,
                    &expected_synth,
                    frames,
                );
                assert_eq!(engine.active_sample_slots, previous_sample_slots);
                assert_eq!(engine.active_synth_slots, previous_synth_slots);
                assert_eq!(
                    runtime.prefix_activity_applies_for_test(),
                    [INSTRUMENT_SLOT_COUNT, INSTRUMENT_SLOT_COUNT]
                );
            }
        }
    }

    fn reduction_scratch(
        frames: usize,
        varied_prefixes: bool,
    ) -> [SourceWorkerScratch; SOURCE_WORKER_COUNT] {
        let mut scratch = std::array::from_fn(|_| SourceWorkerScratch::new());
        let sample_prefixes = if varied_prefixes {
            [frames, frames / 2, 0, frames, frames / 3]
        } else {
            [frames; 5]
        };
        let synth_prefixes = if varied_prefixes {
            [frames / 4, frames, frames / 2, 0, frames]
        } else {
            [frames; 5]
        };
        for (lane, prefix) in sample_prefixes.into_iter().enumerate() {
            set_reduction_lane(&mut scratch, lane, 0, prefix, frames, false);
        }
        for (lane, prefix) in synth_prefixes.into_iter().enumerate() {
            set_reduction_lane(&mut scratch, lane + 5, 1, prefix, frames, true);
        }
        set_reduction_lane(
            &mut scratch,
            10,
            INVALID_INSTRUMENT_SLOT,
            frames,
            frames,
            false,
        );
        set_reduction_lane(
            &mut scratch,
            11,
            INVALID_INSTRUMENT_SLOT,
            frames,
            frames,
            true,
        );
        scratch
    }

    fn set_reduction_lane(
        scratch: &mut [SourceWorkerScratch; SOURCE_WORKER_COUNT],
        lane: usize,
        slot: u8,
        prefix: usize,
        frames: usize,
        synth: bool,
    ) {
        let parity = lane % SOURCE_WORKER_COUNT;
        let local_lane = lane / SOURCE_WORKER_COUNT;
        let (samples, rendered_frames, slots) = if synth {
            let source = &mut scratch[parity].synth;
            (
                &mut source.samples,
                &mut source.rendered_frames,
                &mut source.slots,
            )
        } else {
            let source = &mut scratch[parity].sample;
            (
                &mut source.samples,
                &mut source.rendered_frames,
                &mut source.slots,
            )
        };
        slots[local_lane] = slot;
        rendered_frames[local_lane] = prefix;
        for (frame, sample) in samples[local_lane][..frames].iter_mut().enumerate() {
            *sample = if frame < prefix {
                cancellation_sensitive_value(lane, frame)
            } else {
                0.0
            };
        }
    }

    fn boolean_reference(
        scratch: &[SourceWorkerScratch; SOURCE_WORKER_COUNT],
        frames: usize,
        synth: bool,
    ) -> (
        [Vec<f32>; INSTRUMENT_SLOT_COUNT],
        [Vec<bool>; INSTRUMENT_SLOT_COUNT],
    ) {
        let mut output = std::array::from_fn(|_| vec![0.0; frames]);
        let mut active = std::array::from_fn(|_| vec![false; frames]);
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let parity = lane % SOURCE_WORKER_COUNT;
            let local_lane = lane / SOURCE_WORKER_COUNT;
            let (source_samples, rendered_frames, slots) = if synth {
                (
                    &scratch[parity].synth.samples,
                    &scratch[parity].synth.rendered_frames,
                    &scratch[parity].synth.slots,
                )
            } else {
                (
                    &scratch[parity].sample.samples,
                    &scratch[parity].sample.rendered_frames,
                    &scratch[parity].sample.slots,
                )
            };
            let slot = slots[local_lane];
            if slot == INVALID_INSTRUMENT_SLOT || slot as usize >= INSTRUMENT_SLOT_COUNT {
                continue;
            }
            let slot = slot as usize;
            for frame in 0..frames {
                output[slot][frame] += source_samples[local_lane][frame];
                active[slot][frame] |= frame < rendered_frames[local_lane];
            }
        }
        (output, active)
    }

    fn assert_reduced_matches(
        output: &[Vec<f32>; INSTRUMENT_SLOT_COUNT],
        active: &[Vec<bool>; INSTRUMENT_SLOT_COUNT],
        expected: &(
            [Vec<f32>; INSTRUMENT_SLOT_COUNT],
            [Vec<bool>; INSTRUMENT_SLOT_COUNT],
        ),
        frames: usize,
    ) {
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            for frame in 0..frames {
                assert_eq!(
                    output[slot][frame].to_bits(),
                    expected.0[slot][frame].to_bits(),
                    "slot {slot}, frame {frame}"
                );
                assert_eq!(active[slot][frame], expected.1[slot][frame]);
            }
        }
    }

    fn cancellation_sensitive_value(lane: usize, frame: usize) -> f32 {
        match lane {
            0 | 5 => 16_777_216.0,
            1 | 6 => -16_777_216.0,
            2 | 7 => 1.0,
            3 | 8 => -1.0,
            _ => lane as f32 * 0.125 + frame as f32 * 0.0001,
        }
    }
}
