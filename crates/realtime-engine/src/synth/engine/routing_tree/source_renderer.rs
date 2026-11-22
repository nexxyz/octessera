use super::super::render_voice::synth_voice_render::SynthSourceContext;
use super::super::voice_block_sum::{sum_sample_voice, sum_synth_voice, SlotBlockSum};
use super::super::BLOCK_SLOT_SCRATCH_FRAMES;
use super::source_bank::RoutingTreeSourceBank;
use crate::synth::types::{
    INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
    VOICE_PARTITION_COUNT,
};

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

/// Renders the bank's voices, adding each straight into its instrument's `slot_out` in the inline
/// executor's order (sample voices, then synth voices, every partition's lanes in turn), so both
/// paths round their sums alike.
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
    let mut sample_prefix = 0;
    for lane in partition_order(SAMPLE_VOICE_LANE_CAPACITY) {
        let voice = &mut bank.sample[lane];
        if !voice.active {
            continue;
        }
        let rendered = sum_sample_voice(
            voice,
            frames,
            sample_rate,
            &mut scratch.voice_block,
            &mut sum,
        );
        sample_prefix = sample_prefix.max(rendered);
    }
    for lane in partition_order(SYNTH_VOICE_LANE_CAPACITY) {
        let voice = &mut bank.synth[lane];
        if !voice.active {
            continue;
        }
        if voice.instrument_slot as usize >= INSTRUMENT_SLOT_COUNT {
            voice.active = false;
            continue;
        }
        sum_synth_voice(
            voice,
            bank.rings[lane].as_deref_mut().expect("routing voice ring"),
            frames,
            base_sample_clock,
            synth_context,
            &mut scratch.voice_block,
            &mut sum,
        );
    }
    scratch.rendered_prefix = sum.longest_prefix().max(sample_prefix).min(frames);
}

fn partition_order(lanes: usize) -> impl Iterator<Item = usize> {
    (0..VOICE_PARTITION_COUNT)
        .flat_map(move |parity| (parity..lanes).step_by(VOICE_PARTITION_COUNT))
}
