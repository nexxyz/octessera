use super::routing_tree_source_bank::RoutingTreeSourceBank;
use super::source_lane_renderer::SynthSourceContext;
use super::voice_block_sum::{sum_sample_voice, sum_synth_voice, SlotBlockSum};
use super::BLOCK_SLOT_SCRATCH_FRAMES;
use crate::synth::types::INSTRUMENT_SLOT_COUNT;

pub(super) struct RoutingTreeSourceBlockScratch {
    voice_block: Vec<f32>,
    /// The longest prefix any voice rendered this block: the frames where a source sounded.
    pub(super) rendered_prefix: usize,
}

impl RoutingTreeSourceBlockScratch {
    pub(super) fn new() -> Self {
        Self {
            voice_block: vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES],
            rendered_prefix: 0,
        }
    }

    pub(super) fn prepare(&mut self, frames: usize) -> bool {
        self.rendered_prefix = 0;
        frames <= BLOCK_SLOT_SCRATCH_FRAMES
    }
}

/// Renders the bank's voices, adding each straight into its instrument's `slot_out`.
pub(super) fn render_routing_tree_sources(
    bank: &mut RoutingTreeSourceBank,
    frames: usize,
    base_sample_clock: u64,
    synth_context: &SynthSourceContext,
    sample_rate: u32,
    scratch: &mut RoutingTreeSourceBlockScratch,
    slot_out: &mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
) {
    let mut sum = SlotBlockSum::new(slot_out);
    for (voice, ring) in bank.synth.iter_mut().zip(bank.rings.iter_mut()) {
        if !voice.active {
            continue;
        }
        if voice.instrument_slot as usize >= INSTRUMENT_SLOT_COUNT {
            voice.active = false;
            continue;
        }
        sum_synth_voice(
            voice,
            ring.as_deref_mut().expect("routing voice ring"),
            frames,
            base_sample_clock,
            synth_context,
            &mut scratch.voice_block,
            &mut sum,
        );
    }
    let mut sample_prefix = 0;
    for voice in bank.sample.iter_mut().filter(|voice| voice.active) {
        let rendered = sum_sample_voice(
            voice,
            frames,
            sample_rate,
            &mut scratch.voice_block,
            &mut sum,
        );
        sample_prefix = sample_prefix.max(rendered);
    }
    scratch.rendered_prefix = sum.longest_prefix().max(sample_prefix).min(frames);
}
