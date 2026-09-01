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
            for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
                let parity = lane % VOICE_PARTITION_COUNT;
                let local_lane = lane / VOICE_PARTITION_COUNT;
                if self.synth_scratch[parity].slots[local_lane] as usize != slot {
                    continue;
                }
                for frame in 0..frames {
                    slot_out[slot][frame] += self.synth_scratch[parity].samples[local_lane][frame];
                    slot_active[slot][frame] |=
                        self.synth_scratch[parity].active[local_lane][frame];
                    active_slots[slot] |= self.synth_scratch[parity].active[local_lane][frame];
                }
            }
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
            for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
                let parity = lane % VOICE_PARTITION_COUNT;
                let local_lane = lane / VOICE_PARTITION_COUNT;
                if self.sample_scratch[parity].slots[local_lane] as usize != slot {
                    continue;
                }
                for frame in 0..frames {
                    slot_out[slot][frame] += self.sample_scratch[parity].samples[local_lane][frame];
                    slot_active[slot][frame] |=
                        self.sample_scratch[parity].active[local_lane][frame];
                    active_slots[slot] |= self.sample_scratch[parity].active[local_lane][frame];
                }
            }
        }
    }
}

pub(super) struct SourceRenderOutput<'a> {
    pub slot_out: &'a mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    pub slot_active: &'a mut [Vec<bool>; INSTRUMENT_SLOT_COUNT],
    pub active_slots: &'a mut [bool; INSTRUMENT_SLOT_COUNT],
}
