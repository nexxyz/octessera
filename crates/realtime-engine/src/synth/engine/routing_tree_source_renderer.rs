use super::routing_tree_source_bank::RoutingTreeSourceBank;
use super::source_lane_renderer::{
    render_sample_voice_block, render_synth_voice_block, SynthSourceContext, SynthVoiceFrameContext,
};
use super::BLOCK_SLOT_SCRATCH_FRAMES;
use crate::synth::types::{
    INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
};

pub(super) struct RoutingTreeSourceBlockScratch {
    pub(super) synth_samples: [Vec<f32>; SYNTH_VOICE_LANE_CAPACITY],
    pub(super) sample_samples: [Vec<f32>; SAMPLE_VOICE_LANE_CAPACITY],
    pub(super) synth_rendered_frames: [usize; SYNTH_VOICE_LANE_CAPACITY],
    pub(super) sample_rendered_frames: [usize; SAMPLE_VOICE_LANE_CAPACITY],
}

impl RoutingTreeSourceBlockScratch {
    pub(super) fn new() -> Self {
        Self {
            synth_samples: std::array::from_fn(|_| vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES]),
            sample_samples: std::array::from_fn(|_| vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES]),
            synth_rendered_frames: [0; SYNTH_VOICE_LANE_CAPACITY],
            sample_rendered_frames: [0; SAMPLE_VOICE_LANE_CAPACITY],
        }
    }

    pub(super) fn prepare(&mut self, frames: usize) -> bool {
        if frames > BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        self.synth_rendered_frames.fill(0);
        self.sample_rendered_frames.fill(0);
        true
    }
}

pub(super) fn render_routing_tree_sources(
    bank: &mut RoutingTreeSourceBank,
    frames: usize,
    base_sample_clock: u64,
    synth_context: &SynthSourceContext,
    sample_rate: u32,
    scratch: &mut RoutingTreeSourceBlockScratch,
) {
    for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
        let voice = &mut bank.synth[lane];
        if !voice.active {
            continue;
        }
        let slot = voice.instrument_slot as usize;
        if slot >= INSTRUMENT_SLOT_COUNT {
            voice.active = false;
            voice.canonical_lane = None;
            continue;
        }
        scratch.synth_rendered_frames[lane] = render_synth_voice_block(
            voice,
            slot,
            frames,
            base_sample_clock,
            SynthVoiceFrameContext {
                sample_rate: synth_context.sample_rate,
                config: synth_context.configs[slot],
                render_config: synth_context.render_configs[slot],
                revision: synth_context.revisions[slot],
                mods: synth_context.mods[slot],
            },
            &mut scratch.synth_samples[lane],
        );
    }
    for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
        let voice = &mut bank.sample[lane];
        if !voice.active {
            continue;
        }
        scratch.sample_rendered_frames[lane] = render_sample_voice_block(
            voice,
            frames,
            sample_rate,
            &mut scratch.sample_samples[lane],
        );
    }
}
