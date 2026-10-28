use super::super::synth_voice_pool::SynthVoicePool;
use super::super::types::{INSTRUMENT_SLOT_COUNT, VOICE_PARTITION_COUNT};
use super::render_voice::synth_voice_render::SynthSourceContext;
use super::sample_voice_pool::SampleVoicePool;
use super::sample_voice_renderer::SampleSourceContext;
use super::voice_block_sum::{sum_sample_partition, sum_synth_partition, SlotBlockSum};
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

/// Renders the voice pools on the audio thread, adding each voice straight into its instrument.
pub(super) struct InlineSourceExecutor {
    voice_block: Vec<f32>,
}

impl InlineSourceExecutor {
    pub(super) fn new() -> Self {
        Self {
            voice_block: vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES],
        }
    }

    pub(super) fn prepare(&mut self, frames: usize) -> bool {
        frames <= BLOCK_SLOT_SCRATCH_FRAMES
    }

    pub(super) fn render_synth_sources(
        &mut self,
        frames: usize,
        base_sample_clock: u64,
        pool: &mut SynthVoicePool,
        context: SynthSourceContext,
        output: SourceRenderOutput<'_>,
    ) {
        let mut sum = SlotBlockSum::new(output.slot_out);
        for parity in 0..VOICE_PARTITION_COUNT {
            let Some(mut partition) = pool.take_partition(parity) else {
                continue;
            };
            sum_synth_partition(
                &mut partition,
                frames,
                base_sample_clock,
                &context,
                &mut self.voice_block,
                &mut sum,
            );
            assert!(pool.install_partition(parity, partition).is_ok());
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            pool.compact_slot_lanes(slot);
        }
        apply_rendered_prefixes(sum.rendered_prefixes, frames, output.slot_active);
        #[cfg(test)]
        record_prefix_activity_apply(0);
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
        let mut sum = SlotBlockSum::new(output.slot_out);
        for parity in 0..VOICE_PARTITION_COUNT {
            let Some(mut partition) = pool.take_partition(parity) else {
                continue;
            };
            sum_sample_partition(
                &mut partition,
                frames,
                context.sample_rate,
                &mut self.voice_block,
                &mut sum,
            );
            assert!(pool.install_partition(parity, partition).is_ok());
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            pool.compact_slot_lanes(slot);
        }
        apply_rendered_prefixes(sum.rendered_prefixes, frames, output.slot_active);
        #[cfg(test)]
        record_prefix_activity_apply(1);
        for (slot, active) in output.active_slots.iter_mut().enumerate() {
            *active = pool.active_count_for_slot(slot).unwrap_or(0) > 0;
        }
    }
}

fn apply_rendered_prefixes(
    rendered_prefixes: [usize; INSTRUMENT_SLOT_COUNT],
    frames: usize,
    slot_active: &mut [Vec<bool>; INSTRUMENT_SLOT_COUNT],
) {
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        let rendered_prefix = rendered_prefixes[slot].min(frames);
        slot_active[slot][..rendered_prefix].fill(true);
        slot_active[slot][rendered_prefix..frames].fill(false);
    }
}

pub(super) struct SourceRenderOutput<'a> {
    pub slot_out: &'a mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    pub slot_active: &'a mut [Vec<bool>; INSTRUMENT_SLOT_COUNT],
    pub active_slots: &'a mut [bool; INSTRUMENT_SLOT_COUNT],
}
