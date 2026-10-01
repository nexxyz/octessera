use super::super::super::runtime_state::{InstrumentMod, Voice};
use super::super::super::types::SynthConfig;
use super::{
    prepare_synth_voice_block, refresh_synth_voice_render_cache,
    render_synth_voice_sample_block_precomputed, render_synth_voice_sample_precomputed,
    SynthVoiceRenderConfig, VoiceSource,
};
use crate::synth::pluck_string::PluckRing;

#[derive(Clone, Copy)]
pub(in crate::synth::engine) struct SynthVoiceFrameContext {
    pub(in crate::synth::engine) sample_rate: u32,
    pub(in crate::synth::engine) config: SynthConfig,
    pub(in crate::synth::engine) render_config: SynthVoiceRenderConfig,
    pub(in crate::synth::engine) revision: u32,
    pub(in crate::synth::engine) mods: InstrumentMod,
}

pub(in crate::synth::engine) fn render_synth_voice_frame(
    voice: &mut Voice,
    ring: &mut PluckRing,
    slot_idx: usize,
    frame_sample_clock: u64,
    context: SynthVoiceFrameContext,
) -> Option<f32> {
    if !voice.active {
        return None;
    }
    if !voice_source_matches(voice, &context.render_config) {
        voice.active = false;
        voice.canonical_lane = None;
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
        if let VoiceSource::Fm { index_env, .. } = context.render_config.source {
            voice
                .index_env
                .begin_release(index_env, context.sample_rate);
        }
    }
    let amp_env = voice.amp_env.next();
    let filt_env = voice.filt_env.next();
    if voice.amp_env.is_off() {
        voice.active = false;
        voice.canonical_lane = None;
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
        ring,
        amp_env,
        filt_env,
    ))
}

pub(in crate::synth::engine) fn render_synth_voice_block(
    voice: &mut Voice,
    ring: &mut PluckRing,
    slot_idx: usize,
    frames: usize,
    base_sample_clock: u64,
    context: SynthVoiceFrameContext,
    samples: &mut [f32],
) -> usize {
    if frames == 0 || !voice.active {
        return 0;
    }
    if !voice_source_matches(voice, &context.render_config) {
        voice.active = false;
        voice.canonical_lane = None;
        return 0;
    }
    debug_assert_eq!(voice.instrument_slot as usize, slot_idx);
    if voice.render_revision != context.revision {
        refresh_synth_voice_render_cache(
            voice,
            &context.render_config,
            context.sample_rate,
            context.revision,
        );
    }
    let block = prepare_synth_voice_block(
        &context.render_config,
        context.mods,
        voice.velocity_norm,
        voice.filter_key_scale,
        context.sample_rate,
    );
    let initial_filter = voice.filt;
    if let Some(cutoff) = block.static_cutoff {
        voice.filt.prepare(
            context.render_config.filter_kind,
            cutoff,
            block.q,
            context.sample_rate,
        );
    }
    let mut rendered_frames = 0;
    for (frame, sample) in samples[..frames].iter_mut().enumerate() {
        if base_sample_clock.saturating_add(frame as u64) >= voice.note_off_sample {
            voice
                .amp_env
                .begin_release(context.config.amp_env, context.sample_rate);
            voice
                .filt_env
                .begin_release(context.config.filter_env, context.sample_rate);
            if let VoiceSource::Fm { index_env, .. } = context.render_config.source {
                voice
                    .index_env
                    .begin_release(index_env, context.sample_rate);
            }
        }
        let amp_env = voice.amp_env.next();
        let filt_env = voice.filt_env.next();
        if voice.amp_env.is_off() {
            voice.active = false;
            voice.canonical_lane = None;
            if rendered_frames == 0 && block.static_cutoff.is_some() {
                voice.filt = initial_filter;
            }
            break;
        }
        *sample = render_synth_voice_sample_block_precomputed(
            &context, voice, ring, amp_env, filt_env, block,
        );
        rendered_frames += 1;
    }
    rendered_frames
}

fn voice_source_matches(voice: &Voice, config: &SynthVoiceRenderConfig) -> bool {
    matches!(config.source, VoiceSource::Fm { .. }) == voice.fm
        && matches!(config.source, VoiceSource::Pluck { .. }) == voice.pluck_voice
        && matches!(config.source, VoiceSource::Drum) == voice.drum_voice
        && !matches!(config.source, VoiceSource::Disabled)
        && voice.source_generation == config.source_generation
}
