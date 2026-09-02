use super::super::synth_voice_pool::SynthVoicePool;
use super::super::types::{
    INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
    VOICE_PARTITION_COUNT,
};
use super::sample_voice_pool::SampleVoicePool;
use super::source_lane_renderer::{
    render_sample_partition, render_synth_partition, SampleSourceContext, SourceLaneBlockScratch,
    SynthSourceContext,
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
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let mut rendered_prefix = 0;
            for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
                let parity = lane % VOICE_PARTITION_COUNT;
                let local_lane = lane / VOICE_PARTITION_COUNT;
                if self.synth_scratch[parity].slots[local_lane] as usize != slot {
                    continue;
                }
                rendered_prefix =
                    rendered_prefix.max(self.synth_scratch[parity].rendered_frames[local_lane]);
                for (out, sample) in slot_out[slot][..frames]
                    .iter_mut()
                    .zip(self.synth_scratch[parity].samples[local_lane][..frames].iter())
                {
                    *out += *sample;
                }
            }
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
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let mut rendered_prefix = 0;
            for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
                let parity = lane % VOICE_PARTITION_COUNT;
                let local_lane = lane / VOICE_PARTITION_COUNT;
                if self.sample_scratch[parity].slots[local_lane] as usize != slot {
                    continue;
                }
                rendered_prefix =
                    rendered_prefix.max(self.sample_scratch[parity].rendered_frames[local_lane]);
                for (out, sample) in slot_out[slot][..frames]
                    .iter_mut()
                    .zip(self.sample_scratch[parity].samples[local_lane][..frames].iter())
                {
                    *out += *sample;
                }
            }
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
    use crate::synth::engine::source_lane_renderer::INVALID_INSTRUMENT_SLOT;

    #[test]
    fn prefix_reduction_matches_boolean_activity_and_lane_order() {
        for frames in [32, 64, 128, 256, 2048] {
            for varied_prefixes in [false, true] {
                let mut executor = InlineSourceExecutor::new();
                let prefixes = if varied_prefixes {
                    [frames, frames / 2, 0, frames / 3]
                } else {
                    [frames; 4]
                };
                for (lane, prefix) in prefixes.into_iter().enumerate() {
                    set_synth_lane(&mut executor, lane, prefix, frames);
                }
                set_synth_lane(&mut executor, 4, 0, frames);
                executor.synth_scratch[0].slots[5] = INVALID_INSTRUMENT_SLOT;
                executor.synth_scratch[0].rendered_frames[5] = frames;

                let mut expected_output = vec![0.0; frames];
                let mut expected_active = vec![false; frames];
                for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
                    let parity = lane % VOICE_PARTITION_COUNT;
                    let local_lane = lane / VOICE_PARTITION_COUNT;
                    let scratch = &executor.synth_scratch[parity];
                    if scratch.slots[local_lane] != 0 {
                        continue;
                    }
                    for frame in 0..frames {
                        expected_output[frame] += scratch.samples[local_lane][frame];
                        expected_active[frame] |= frame < scratch.rendered_frames[local_lane];
                    }
                }

                let mut slot_out = std::array::from_fn(|_| vec![0.0; frames]);
                let mut slot_active = std::array::from_fn(|_| vec![false; frames]);
                let mut active_slots = [false; INSTRUMENT_SLOT_COUNT];
                executor.reduce_synth_sources(
                    frames,
                    &mut slot_out,
                    &mut slot_active,
                    &mut active_slots,
                );

                for frame in 0..frames {
                    assert_eq!(
                        slot_out[0][frame].to_bits(),
                        expected_output[frame].to_bits()
                    );
                    assert_eq!(slot_active[0][frame], expected_active[frame]);
                }
                assert_eq!(
                    active_slots[0],
                    expected_active.iter().any(|active| *active)
                );
            }
        }
    }

    fn set_synth_lane(
        executor: &mut InlineSourceExecutor,
        lane: usize,
        prefix: usize,
        frames: usize,
    ) {
        let parity = lane % VOICE_PARTITION_COUNT;
        let local_lane = lane / VOICE_PARTITION_COUNT;
        let scratch = &mut executor.synth_scratch[parity];
        scratch.slots[local_lane] = 0;
        scratch.rendered_frames[local_lane] = prefix;
        for (frame, sample) in scratch.samples[local_lane][..frames].iter_mut().enumerate() {
            *sample = if frame < prefix {
                match lane {
                    0 => 16_777_216.0,
                    1 => -16_777_216.0,
                    2 => 1.0,
                    _ => lane as f32 * 0.125,
                }
            } else {
                0.0
            };
        }
    }
}
