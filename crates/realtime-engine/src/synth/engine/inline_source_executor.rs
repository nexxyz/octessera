use super::super::synth_voice_pool::SynthVoicePool;
use super::super::types::{
    INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
    VOICE_PARTITION_COUNT,
};
use super::sample_voice_pool::SampleVoicePool;
use super::source_lane_renderer::{
    render_sample_partition, render_synth_partition, SampleSourceContext, SourceLaneBlockScratch,
    SynthSourceContext, INVALID_INSTRUMENT_SLOT,
};
use super::BLOCK_SLOT_SCRATCH_FRAMES;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static PREFIX_ACTIVITY_APPLIES: Cell<[usize; 2]> = const { Cell::new([0; 2]) };
}

#[cfg(test)]
fn record_prefix_activity_apply(index: usize) {
    PREFIX_ACTIVITY_APPLIES.with(|applies| {
        let mut value = applies.get();
        value[index] += 1;
        applies.set(value);
    });
}

#[cfg(test)]
pub(super) fn prefix_activity_applies_for_test() -> [usize; 2] {
    PREFIX_ACTIVITY_APPLIES.with(Cell::get)
}

#[cfg(test)]
pub(super) fn reset_prefix_activity_applies_for_test() {
    PREFIX_ACTIVITY_APPLIES.with(|applies| applies.set([0; 2]));
}

pub(super) struct InlineSourceExecutor {
    synth_scratch: [SourceLaneBlockScratch; VOICE_PARTITION_COUNT],
    sample_scratch: [SourceLaneBlockScratch; VOICE_PARTITION_COUNT],
}

impl InlineSourceExecutor {
    pub(super) fn new() -> Self {
        Self {
            synth_scratch: std::array::from_fn(|_| SourceLaneBlockScratch::new()),
            sample_scratch: std::array::from_fn(|_| SourceLaneBlockScratch::new()),
        }
    }

    pub(super) fn prepare(&mut self, frames: usize) -> bool {
        if frames > BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        for scratch in self
            .synth_scratch
            .iter_mut()
            .chain(self.sample_scratch.iter_mut())
        {
            if !scratch.prepare(frames) {
                return false;
            }
        }
        true
    }

    pub(super) fn into_partition_scratch(
        self,
    ) -> (
        [SourceLaneBlockScratch; VOICE_PARTITION_COUNT],
        [SourceLaneBlockScratch; VOICE_PARTITION_COUNT],
    ) {
        (self.synth_scratch, self.sample_scratch)
    }

    pub(super) fn render_synth_sources(
        &mut self,
        frames: usize,
        base_sample_clock: u64,
        pool: &mut SynthVoicePool,
        context: SynthSourceContext,
        output: SourceRenderOutput<'_>,
    ) {
        for parity in 0..VOICE_PARTITION_COUNT {
            let Some(mut partition) = pool.take_partition(parity) else {
                continue;
            };
            render_synth_partition(
                &mut partition,
                frames,
                base_sample_clock,
                &context,
                &mut self.synth_scratch[parity],
            );
            assert!(pool.install_partition(parity, partition).is_ok());
        }
        for (slot, active) in output.active_slots.iter_mut().enumerate() {
            pool.compact_slot_lanes(slot);
            *active = false;
        }
        self.reduce_synth_sources(
            frames,
            &mut *output.slot_out,
            &mut *output.slot_active,
            &mut *output.active_slots,
        );
        for (slot, active) in output.active_slots.iter_mut().enumerate() {
            *active = pool.active_count_for_slot(slot).unwrap_or(0) > 0;
        }
    }

    pub(super) fn render_sample_sources(
        &mut self,
        frames: usize,
        pool: &mut SampleVoicePool,
        context: SampleSourceContext,
        output: SourceRenderOutput<'_>,
    ) {
        for parity in 0..VOICE_PARTITION_COUNT {
            let Some(mut partition) = pool.take_partition(parity) else {
                continue;
            };
            render_sample_partition(
                &mut partition,
                frames,
                context,
                &mut self.sample_scratch[parity],
            );
            assert!(pool.install_partition(parity, partition).is_ok());
        }
        for (slot, active) in output.active_slots.iter_mut().enumerate() {
            pool.compact_slot_lanes(slot);
            *active = false;
        }
        self.reduce_sample_sources(
            frames,
            &mut *output.slot_out,
            &mut *output.slot_active,
            &mut *output.active_slots,
        );
        for (slot, active) in output.active_slots.iter_mut().enumerate() {
            *active = pool.active_count_for_slot(slot).unwrap_or(0) > 0;
        }
    }

    fn reduce_synth_sources(
        &self,
        frames: usize,
        slot_out: &mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
        slot_active: &mut [Vec<bool>; INSTRUMENT_SLOT_COUNT],
        active_slots: &mut [bool; INSTRUMENT_SLOT_COUNT],
    ) {
        let mut rendered_prefixes = [0; INSTRUMENT_SLOT_COUNT];
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let parity = lane % VOICE_PARTITION_COUNT;
            let local_lane = lane / VOICE_PARTITION_COUNT;
            let scratch = &self.synth_scratch[parity];
            let raw_slot = scratch.slots[local_lane];
            if raw_slot == INVALID_INSTRUMENT_SLOT || raw_slot as usize >= INSTRUMENT_SLOT_COUNT {
                continue;
            }
            let slot = raw_slot as usize;
            let lane_rendered_frames = scratch.rendered_frames[local_lane].min(frames);
            rendered_prefixes[slot] = rendered_prefixes[slot].max(lane_rendered_frames);
            for (out, sample) in slot_out[slot][..lane_rendered_frames]
                .iter_mut()
                .zip(scratch.samples[local_lane][..lane_rendered_frames].iter())
            {
                *out += *sample;
            }
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let rendered_prefix = rendered_prefixes[slot];
            slot_active[slot][..rendered_prefix].fill(true);
            slot_active[slot][rendered_prefix..frames].fill(false);
            active_slots[slot] |= rendered_prefix > 0;
            #[cfg(test)]
            record_prefix_activity_apply(0);
        }
    }

    fn reduce_sample_sources(
        &self,
        frames: usize,
        slot_out: &mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
        slot_active: &mut [Vec<bool>; INSTRUMENT_SLOT_COUNT],
        active_slots: &mut [bool; INSTRUMENT_SLOT_COUNT],
    ) {
        let mut rendered_prefixes = [0; INSTRUMENT_SLOT_COUNT];
        for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
            let parity = lane % VOICE_PARTITION_COUNT;
            let local_lane = lane / VOICE_PARTITION_COUNT;
            let scratch = &self.sample_scratch[parity];
            let raw_slot = scratch.slots[local_lane];
            if raw_slot == INVALID_INSTRUMENT_SLOT || raw_slot as usize >= INSTRUMENT_SLOT_COUNT {
                continue;
            }
            let slot = raw_slot as usize;
            let lane_rendered_frames = scratch.rendered_frames[local_lane].min(frames);
            rendered_prefixes[slot] = rendered_prefixes[slot].max(lane_rendered_frames);
            for (out, sample) in slot_out[slot][..lane_rendered_frames]
                .iter_mut()
                .zip(scratch.samples[local_lane][..lane_rendered_frames].iter())
            {
                *out += *sample;
            }
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let rendered_prefix = rendered_prefixes[slot];
            slot_active[slot][..rendered_prefix].fill(true);
            slot_active[slot][rendered_prefix..frames].fill(false);
            active_slots[slot] |= rendered_prefix > 0;
            #[cfg(test)]
            record_prefix_activity_apply(1);
        }
    }
}

pub(super) struct SourceRenderOutput<'a> {
    pub slot_out: &'a mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    pub slot_active: &'a mut [Vec<bool>; INSTRUMENT_SLOT_COUNT],
    pub active_slots: &'a mut [bool; INSTRUMENT_SLOT_COUNT],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_reduction_matches_boolean_activity_and_lane_order() {
        for frames in [32, 64, 128, 256, 2048] {
            for varied_prefixes in [false, true] {
                let mut executor = InlineSourceExecutor::new();
                for scratch in &mut executor.synth_scratch {
                    for samples in &mut scratch.samples {
                        samples[..frames].fill(10_000_000.0);
                    }
                }
                assert!(executor.prepare(frames));
                let prefixes = if varied_prefixes {
                    [frames, frames / 2, 0, frames + 1]
                } else {
                    [frames; 4]
                };
                let slots = [0, 0, 0, 1];
                for (lane, prefix) in prefixes.into_iter().enumerate() {
                    set_synth_lane(&mut executor, lane, slots[lane], prefix, frames);
                }
                set_synth_lane(&mut executor, 4, 1, 0, frames);
                executor.synth_scratch[0].slots[5] = INVALID_INSTRUMENT_SLOT;
                executor.synth_scratch[0].rendered_frames[5] = frames;
                executor.synth_scratch[1].slots[6] = INSTRUMENT_SLOT_COUNT as u8 + 1;
                executor.synth_scratch[1].rendered_frames[6] = frames + 1;

                let mut expected_output: [Vec<f32>; INSTRUMENT_SLOT_COUNT] =
                    std::array::from_fn(|_| vec![0.0; frames]);
                let mut expected_active: [Vec<bool>; INSTRUMENT_SLOT_COUNT] =
                    std::array::from_fn(|_| vec![false; frames]);
                for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
                    let parity = lane % VOICE_PARTITION_COUNT;
                    let local_lane = lane / VOICE_PARTITION_COUNT;
                    let scratch = &executor.synth_scratch[parity];
                    let raw_slot = scratch.slots[local_lane];
                    if raw_slot == INVALID_INSTRUMENT_SLOT
                        || raw_slot as usize >= INSTRUMENT_SLOT_COUNT
                    {
                        continue;
                    }
                    let slot = raw_slot as usize;
                    let rendered_frames = scratch.rendered_frames[local_lane].min(frames);
                    for frame in 0..rendered_frames {
                        expected_output[slot][frame] += scratch.samples[local_lane][frame];
                        expected_active[slot][frame] = true;
                    }
                }

                let mut slot_out = std::array::from_fn(|_| vec![0.0; frames]);
                let mut slot_active = std::array::from_fn(|_| vec![false; frames]);
                let mut active_slots = [false; INSTRUMENT_SLOT_COUNT];
                reset_prefix_activity_applies_for_test();
                executor.reduce_synth_sources(
                    frames,
                    &mut slot_out,
                    &mut slot_active,
                    &mut active_slots,
                );

                for slot in 0..INSTRUMENT_SLOT_COUNT {
                    for frame in 0..frames {
                        assert_eq!(
                            slot_out[slot][frame].to_bits(),
                            expected_output[slot][frame].to_bits()
                        );
                        assert_eq!(slot_active[slot][frame], expected_active[slot][frame]);
                    }
                    assert_eq!(
                        active_slots[slot],
                        expected_active[slot].iter().any(|active| *active)
                    );
                }
                assert_eq!(
                    prefix_activity_applies_for_test(),
                    [INSTRUMENT_SLOT_COUNT, 0]
                );
            }
        }
    }

    #[test]
    fn sample_prefix_reduction_handles_multiple_slots_invalid_metadata_and_stale_tail() {
        let frames = 64;
        let mut executor = InlineSourceExecutor::new();
        for scratch in &mut executor.sample_scratch {
            for samples in &mut scratch.samples {
                samples[..frames].fill(7.0);
            }
        }
        assert!(executor.prepare(frames));
        let scratch = &mut executor.sample_scratch[0];
        scratch.slots[0] = 0;
        scratch.rendered_frames[0] = 1;
        scratch.samples[0][0] = 1.0;
        scratch.slots[1] = 1;
        scratch.rendered_frames[1] = frames + 1;
        scratch.samples[1][..frames].fill(2.0);
        scratch.slots[2] = INVALID_INSTRUMENT_SLOT;
        scratch.rendered_frames[2] = frames;
        scratch.slots[3] = INSTRUMENT_SLOT_COUNT as u8 + 1;
        scratch.rendered_frames[3] = frames;

        let mut slot_out = std::array::from_fn(|_| vec![0.0; frames]);
        slot_out[6][0] = 9.0;
        let mut slot_active = std::array::from_fn(|_| vec![false; frames]);
        let mut active_slots = [false; INSTRUMENT_SLOT_COUNT];
        active_slots[6] = true;
        reset_prefix_activity_applies_for_test();
        executor.reduce_sample_sources(frames, &mut slot_out, &mut slot_active, &mut active_slots);

        assert_eq!(slot_out[0][0], 1.0);
        assert!(slot_out[0][1..].iter().all(|sample| *sample == 0.0));
        assert!(slot_active[0][0]);
        assert!(slot_active[0][1..].iter().all(|active| !active));
        assert!(active_slots[0]);
        assert!(slot_out[1].iter().all(|sample| *sample == 2.0));
        assert!(slot_active[1].iter().all(|active| *active));
        assert!(active_slots[1]);
        for slot in [2, 3, 4, 5, 7] {
            assert!(slot_out[slot].iter().all(|sample| *sample == 0.0));
            assert!(slot_active[slot].iter().all(|active| !active));
            assert!(!active_slots[slot]);
        }
        assert_eq!(slot_out[6][0], 9.0);
        assert!(active_slots[6]);
        assert!(slot_active[6].iter().all(|active| !active));
        assert_eq!(
            prefix_activity_applies_for_test(),
            [0, INSTRUMENT_SLOT_COUNT]
        );
    }

    fn set_synth_lane(
        executor: &mut InlineSourceExecutor,
        lane: usize,
        slot: u8,
        prefix: usize,
        frames: usize,
    ) {
        let parity = lane % VOICE_PARTITION_COUNT;
        let local_lane = lane / VOICE_PARTITION_COUNT;
        let scratch = &mut executor.synth_scratch[parity];
        scratch.slots[local_lane] = slot;
        scratch.rendered_frames[local_lane] = prefix;
        for sample in scratch.samples[local_lane][..frames]
            .iter_mut()
            .take(prefix)
        {
            *sample = match lane {
                0 => 16_777_216.0,
                1 => -16_777_216.0,
                2 => 1.0,
                _ => lane as f32 * 0.125,
            };
        }
    }
}
