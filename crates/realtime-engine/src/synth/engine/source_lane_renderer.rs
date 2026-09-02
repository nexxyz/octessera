use super::super::runtime_state::{BiquadState, InstrumentMod, Voice};
use super::super::synth_voice_pool::SynthVoicePartition;
use super::super::types::{
    FilterType, SynthConfig, INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY,
    SAMPLE_VOICE_PARTITION_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
    SYNTH_VOICE_PARTITION_LANE_CAPACITY, VOICE_PARTITION_COUNT,
};
use super::render_voice::{
    prepare_synth_voice_block, refresh_synth_voice_render_cache,
    render_synth_voice_sample_block_precomputed, render_synth_voice_sample_precomputed,
    SynthVoiceRenderConfig,
};
use super::sample_voice_pool::SampleVoicePartition;
use super::support::{mono_frame, SampleVoice};
use super::BLOCK_SLOT_SCRATCH_FRAMES;

const _: () = assert!(
    VOICE_PARTITION_COUNT * SYNTH_VOICE_PARTITION_LANE_CAPACITY == SYNTH_VOICE_LANE_CAPACITY
);
const _: () = assert!(
    VOICE_PARTITION_COUNT * SAMPLE_VOICE_PARTITION_LANE_CAPACITY == SAMPLE_VOICE_LANE_CAPACITY
);
const _: () = assert!(SYNTH_VOICE_PARTITION_LANE_CAPACITY == SAMPLE_VOICE_PARTITION_LANE_CAPACITY);

pub(super) struct SourceLaneBlockScratch {
    pub(super) samples: [Vec<f32>; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
    pub(super) active: [Vec<bool>; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
    pub(super) slots: [u8; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
}

pub(super) const INVALID_INSTRUMENT_SLOT: u8 = INSTRUMENT_SLOT_COUNT as u8;

impl SourceLaneBlockScratch {
    pub(super) fn new() -> Self {
        Self {
            samples: std::array::from_fn(|_| vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES]),
            active: std::array::from_fn(|_| vec![false; BLOCK_SLOT_SCRATCH_FRAMES]),
            slots: [INVALID_INSTRUMENT_SLOT; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
        }
    }

    pub(super) fn prepare(&mut self, frames: usize) -> bool {
        if frames > BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        for samples in &mut self.samples {
            samples[..frames].fill(0.0);
        }
        for active in &mut self.active {
            active[..frames].fill(false);
        }
        self.slots.fill(INVALID_INSTRUMENT_SLOT);
        true
    }
}

pub(super) struct SynthSourceContext {
    pub(super) sample_rate: u32,
    pub(super) configs: [SynthConfig; INSTRUMENT_SLOT_COUNT],
    pub(super) render_configs: [SynthVoiceRenderConfig; INSTRUMENT_SLOT_COUNT],
    pub(super) revisions: [u32; INSTRUMENT_SLOT_COUNT],
    pub(super) mods: [InstrumentMod; INSTRUMENT_SLOT_COUNT],
}

#[derive(Clone, Copy)]
pub(super) struct SampleSourceContext {
    pub(super) sample_rate: u32,
}

pub(super) fn render_synth_partition(
    partition: &mut SynthVoicePartition,
    frames: usize,
    base_sample_clock: u64,
    context: &SynthSourceContext,
    scratch: &mut SourceLaneBlockScratch,
) {
    for (lane, voice) in partition.lanes_mut().iter_mut().enumerate() {
        if !voice.active {
            scratch.slots[lane] = INVALID_INSTRUMENT_SLOT;
            continue;
        }
        let slot = voice.instrument_slot as usize;
        if slot >= INSTRUMENT_SLOT_COUNT {
            scratch.slots[lane] = INVALID_INSTRUMENT_SLOT;
            continue;
        }
        scratch.slots[lane] = slot as u8;
        let frame_context = SynthVoiceFrameContext {
            sample_rate: context.sample_rate,
            config: context.configs[slot],
            render_config: context.render_configs[slot],
            revision: context.revisions[slot],
            mods: context.mods[slot],
        };
        render_synth_voice_block(
            voice,
            slot,
            frames,
            base_sample_clock,
            frame_context,
            &mut scratch.samples[lane],
            &mut scratch.active[lane],
        );
    }
}

pub(super) fn render_sample_partition(
    partition: &mut SampleVoicePartition,
    frames: usize,
    context: SampleSourceContext,
    scratch: &mut SourceLaneBlockScratch,
) {
    for (lane, voice) in partition.lanes_mut().iter_mut().enumerate() {
        if !voice.active {
            scratch.slots[lane] = INVALID_INSTRUMENT_SLOT;
            continue;
        }
        let slot = voice.instrument_slot as usize;
        if slot >= INSTRUMENT_SLOT_COUNT {
            scratch.slots[lane] = INVALID_INSTRUMENT_SLOT;
            continue;
        }
        scratch.slots[lane] = slot as u8;
        render_sample_voice_block(
            voice,
            frames,
            context.sample_rate,
            &mut scratch.samples[lane],
            &mut scratch.active[lane],
        );
    }
}

#[derive(Clone, Copy)]
pub(super) struct SynthVoiceFrameContext {
    pub(super) sample_rate: u32,
    pub(super) config: SynthConfig,
    pub(super) render_config: SynthVoiceRenderConfig,
    pub(super) revision: u32,
    pub(super) mods: InstrumentMod,
}

pub(super) fn render_synth_voice_frame(
    voice: &mut Voice,
    slot_idx: usize,
    frame_sample_clock: u64,
    context: SynthVoiceFrameContext,
) -> Option<f32> {
    if !voice.active {
        return None;
    }
    debug_assert_eq!(voice.instrument_slot as usize, slot_idx);
    if frame_sample_clock >= voice.note_off_sample {
        voice
            .amp_env
            .begin_release(context.config.amp_env, context.sample_rate);
        voice
            .filt_env
            .begin_release(context.config.filter_env, context.sample_rate);
    }
    let amp_env = voice.amp_env.next();
    let filt_env = voice.filt_env.next();
    if voice.amp_env.is_off() {
        voice.active = false;
        return None;
    }
    if voice.render_revision != context.revision {
        refresh_synth_voice_render_cache(
            voice,
            &context.render_config,
            context.sample_rate,
            context.revision,
        );
    }
    Some(render_synth_voice_sample_precomputed(
        context.sample_rate,
        context.mods,
        &context.render_config,
        voice,
        amp_env,
        filt_env,
    ))
}

fn render_synth_voice_block(
    voice: &mut Voice,
    slot_idx: usize,
    frames: usize,
    base_sample_clock: u64,
    context: SynthVoiceFrameContext,
    samples: &mut [f32],
    active: &mut [bool],
) {
    if frames == 0 || !voice.active {
        return;
    }
    debug_assert_eq!(voice.instrument_slot as usize, slot_idx);
    let block =
        prepare_synth_voice_block(&context.render_config, context.mods, voice.velocity_norm);
    let initial_filter = voice.filt;
    if let Some(cutoff) = block.static_cutoff {
        voice.filt.prepare(
            context.render_config.filter_kind,
            cutoff,
            block.q,
            context.sample_rate,
        );
    }
    let mut rendered = false;
    for frame in 0..frames {
        if base_sample_clock.saturating_add(frame as u64) >= voice.note_off_sample {
            voice
                .amp_env
                .begin_release(context.config.amp_env, context.sample_rate);
            voice
                .filt_env
                .begin_release(context.config.filter_env, context.sample_rate);
        }
        let amp_env = voice.amp_env.next();
        let filt_env = voice.filt_env.next();
        if voice.amp_env.is_off() {
            voice.active = false;
            if !rendered && block.static_cutoff.is_some() {
                voice.filt = initial_filter;
            }
            break;
        }
        if voice.render_revision != context.revision {
            refresh_synth_voice_render_cache(
                voice,
                &context.render_config,
                context.sample_rate,
                context.revision,
            );
        }
        samples[frame] = render_synth_voice_sample_block_precomputed(
            context.sample_rate,
            context.mods,
            &context.render_config,
            voice,
            amp_env,
            filt_env,
            block,
        );
        active[frame] = true;
        rendered = true;
    }
}

pub(super) fn render_sample_voice_frame(voice: &mut SampleVoice, sample_rate: u32) -> Option<f32> {
    if !voice.active {
        return None;
    }
    let Some(buffer) = voice.buffer.as_ref() else {
        voice.active = false;
        return None;
    };
    let frames = buffer.samples.len() / buffer.channels as usize;
    if frames == 0 || voice.pos >= frames as f32 {
        voice.active = false;
        return None;
    }
    let frame = voice.pos.floor() as usize;
    let frac = voice.pos - frame as f32;
    let next_frame = (frame + 1).min(frames - 1);
    let sample = mono_frame(buffer, frame) * (1.0 - frac) + mono_frame(buffer, next_frame) * frac;
    let filtered = sample_lowpass(
        sample,
        &mut voice.filt,
        voice.filter_cutoff_hz,
        voice.filter_resonance,
        sample_rate,
    );
    voice.pos += voice.step;
    Some(filtered * voice.gain)
}

pub(super) fn render_sample_voice_block(
    voice: &mut SampleVoice,
    frames: usize,
    sample_rate: u32,
    samples: &mut [f32],
    active: &mut [bool],
) {
    if frames == 0 || !voice.active {
        return;
    }
    let Some(buffer) = voice.buffer.as_ref() else {
        voice.active = false;
        return;
    };
    let buffer_frames = buffer.samples.len() / buffer.channels as usize;
    if buffer_frames == 0 || voice.pos >= buffer_frames as f32 {
        voice.active = false;
        return;
    }
    let q = sample_filter_q(voice.filter_resonance);
    voice
        .filt
        .prepare(FilterType::Lowpass, voice.filter_cutoff_hz, q, sample_rate);
    for frame in 0..frames {
        if let Some(sample) = render_sample_voice_frame_prepared(voice) {
            samples[frame] = sample;
            active[frame] = true;
        } else if !voice.active {
            break;
        }
    }
}

fn render_sample_voice_frame_prepared(voice: &mut SampleVoice) -> Option<f32> {
    if !voice.active {
        return None;
    }
    let Some(buffer) = voice.buffer.as_ref() else {
        voice.active = false;
        return None;
    };
    let frames = buffer.samples.len() / buffer.channels as usize;
    if frames == 0 || voice.pos >= frames as f32 {
        voice.active = false;
        return None;
    }
    let frame = voice.pos.floor() as usize;
    let frac = voice.pos - frame as f32;
    let next_frame = (frame + 1).min(frames - 1);
    let sample = mono_frame(buffer, frame) * (1.0 - frac) + mono_frame(buffer, next_frame) * frac;
    let filtered = voice.filt.process_prepared(sample).clamp(-8.0, 8.0);
    voice.pos += voice.step;
    Some(filtered * voice.gain)
}

pub(super) fn sample_lowpass(
    sample: f32,
    filt: &mut BiquadState,
    cutoff_hz: f32,
    resonance: f32,
    sample_rate: u32,
) -> f32 {
    let q = sample_filter_q(resonance);
    filt.process(sample, FilterType::Lowpass, cutoff_hz, q, sample_rate)
        .clamp(-8.0, 8.0)
}

fn sample_filter_q(resonance: f32) -> f32 {
    0.5 + (resonance.clamp(0.0, 100.0) / 100.0) * 11.5
}

#[cfg(test)]
mod tests {
    use super::super::inline_source_executor::InlineSourceExecutor;
    use super::*;
    use std::mem::size_of;

    const GLOBAL_SYNTH_SCRATCH_BYTES: usize = size_of::<[Vec<f32>; SYNTH_VOICE_LANE_CAPACITY]>()
        + size_of::<[Vec<bool>; SYNTH_VOICE_LANE_CAPACITY]>()
        + size_of::<[u8; SYNTH_VOICE_LANE_CAPACITY]>();
    const GLOBAL_SAMPLE_SCRATCH_BYTES: usize = size_of::<[Vec<f32>; SAMPLE_VOICE_LANE_CAPACITY]>()
        + size_of::<[Vec<bool>; SAMPLE_VOICE_LANE_CAPACITY]>()
        + size_of::<[u8; SAMPLE_VOICE_LANE_CAPACITY]>();

    const _: () = assert!(
        size_of::<[SourceLaneBlockScratch; VOICE_PARTITION_COUNT]>() == GLOBAL_SYNTH_SCRATCH_BYTES
    );
    const _: () = assert!(
        size_of::<[SourceLaneBlockScratch; VOICE_PARTITION_COUNT]>() == GLOBAL_SAMPLE_SCRATCH_BYTES
    );
    const _: () = assert!(
        size_of::<InlineSourceExecutor>()
            == GLOBAL_SYNTH_SCRATCH_BYTES + GLOBAL_SAMPLE_SCRATCH_BYTES
    );

    #[test]
    fn partition_scratch_matches_global_lane_shape() {
        let scratch: [SourceLaneBlockScratch; VOICE_PARTITION_COUNT] =
            std::array::from_fn(|_| SourceLaneBlockScratch::new());
        assert_eq!(
            scratch.len() * SYNTH_VOICE_PARTITION_LANE_CAPACITY,
            SYNTH_VOICE_LANE_CAPACITY
        );
        assert_eq!(
            scratch.len() * SYNTH_VOICE_PARTITION_LANE_CAPACITY,
            SAMPLE_VOICE_LANE_CAPACITY
        );
        assert_eq!(
            size_of::<[SourceLaneBlockScratch; VOICE_PARTITION_COUNT]>(),
            GLOBAL_SYNTH_SCRATCH_BYTES
        );
        assert_eq!(
            size_of::<[SourceLaneBlockScratch; VOICE_PARTITION_COUNT]>(),
            GLOBAL_SAMPLE_SCRATCH_BYTES
        );
        assert_eq!(
            size_of::<InlineSourceExecutor>(),
            GLOBAL_SYNTH_SCRATCH_BYTES + GLOBAL_SAMPLE_SCRATCH_BYTES
        );
    }

    #[test]
    fn scratch_prepare_invalidates_every_lane_slot() {
        let mut scratch = SourceLaneBlockScratch::new();
        scratch.slots.fill(0);
        assert!(scratch.prepare(64));
        assert!(scratch
            .slots
            .iter()
            .all(|slot| *slot == INVALID_INSTRUMENT_SLOT));
    }
}
