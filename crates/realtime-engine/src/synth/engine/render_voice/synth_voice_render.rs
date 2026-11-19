use super::super::super::runtime_state::{InstrumentMod, Voice};
use super::super::super::types::{EnvConfig, SynthConfig, INSTRUMENT_SLOT_COUNT};
use super::{
    fm_source_sample, pluck_source_sample, prepare_synth_voice_block,
    refresh_synth_voice_render_cache, render_synth_voice_sample_precomputed,
    shape_synth_voice_block_sample, synth_source_sample, SynthVoiceBlockRenderState,
    SynthVoiceRenderConfig, VoiceSource,
};
use crate::synth::engine::instrument_param_glide::LevelRamp;
use crate::synth::pluck_string::PluckRing;

#[derive(Clone, Copy)]
pub(in crate::synth::engine) struct SynthSourceContext {
    pub(in crate::synth::engine) sample_rate: u32,
    pub(in crate::synth::engine) configs: [SynthConfig; INSTRUMENT_SLOT_COUNT],
    pub(in crate::synth::engine) render_configs: [SynthVoiceRenderConfig; INSTRUMENT_SLOT_COUNT],
    pub(in crate::synth::engine) revisions: [u32; INSTRUMENT_SLOT_COUNT],
    pub(in crate::synth::engine) mods: [InstrumentMod; INSTRUMENT_SLOT_COUNT],
    pub(in crate::synth::engine) level_ramps: [Option<LevelRamp>; INSTRUMENT_SLOT_COUNT],
}

#[derive(Clone, Copy)]
pub(in crate::synth::engine) struct SynthVoiceFrameContext {
    pub(in crate::synth::engine) sample_rate: u32,
    pub(in crate::synth::engine) config: SynthConfig,
    pub(in crate::synth::engine) render_config: SynthVoiceRenderConfig,
    pub(in crate::synth::engine) revision: u32,
    pub(in crate::synth::engine) mods: InstrumentMod,
    pub(in crate::synth::engine) level_ramp: Option<LevelRamp>,
}

pub(in crate::synth::engine) fn render_synth_voice_frame(
    voice: &mut Voice,
    ring: &mut PluckRing,
    slot_idx: usize,
    frame_sample_clock: u64,
    context: SynthVoiceFrameContext,
) -> Option<f32> {
    let value = render_source_frame(voice, ring, slot_idx, frame_sample_clock, context)?;
    let value = voice
        .declick
        .finish_frame(&mut voice.last_output, value, context.sample_rate);
    if value.is_none() {
        voice.active = false;
    }
    value
}

fn render_source_frame(
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
        return None;
    }
    debug_assert_eq!(voice.instrument_slot as usize, slot_idx);
    if frame_sample_clock < voice.started_sample {
        return Some(0.0);
    }
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
    let rendered = render_source_block(
        voice,
        ring,
        slot_idx,
        frames,
        base_sample_clock,
        context,
        samples,
    );
    let Some(faded) = voice.declick.finish_block(
        &mut voice.last_output,
        &mut samples[..rendered],
        context.sample_rate,
    ) else {
        return rendered;
    };
    voice.active = false;
    faded
}

fn render_source_block(
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
        return 0;
    }
    debug_assert_eq!(voice.instrument_slot as usize, slot_idx);
    let lead = voice.started_sample.saturating_sub(base_sample_clock);
    if lead > 0 {
        let lead = lead.min(frames as u64) as usize;
        samples[..lead].fill(0.0);
        let mut context = context;
        context.level_ramp = context.level_ramp.map(|ramp| ramp.skipped(lead));
        return lead
            + render_started_source_block(
                voice,
                ring,
                frames - lead,
                base_sample_clock + lead as u64,
                context,
                &mut samples[lead..],
            );
    }
    render_started_source_block(voice, ring, frames, base_sample_clock, context, samples)
}

fn render_started_source_block(
    voice: &mut Voice,
    ring: &mut PluckRing,
    frames: usize,
    base_sample_clock: u64,
    context: SynthVoiceFrameContext,
    samples: &mut [f32],
) -> usize {
    if frames == 0 {
        return 0;
    }
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
        voice,
        frames,
        context.sample_rate,
    );
    if let Some(cutoff) = block.static_cutoff {
        let initial_filter = voice.filt;
        voice.filt.prepare(
            context.render_config.filter_kind,
            cutoff,
            block.q,
            context.sample_rate,
        );
        let rendered = render_voice_frames(
            voice,
            ring,
            frames,
            base_sample_clock,
            &context,
            samples,
            block,
        );
        if rendered == 0 {
            voice.filt = initial_filter;
        }
        return rendered;
    }
    render_voice_frames(
        voice,
        ring,
        frames,
        base_sample_clock,
        &context,
        samples,
        block,
    )
}

/// Picks the instrument once per voice and block, so each source runs its own compiled loop.
fn render_voice_frames(
    voice: &mut Voice,
    ring: &mut PluckRing,
    frames: usize,
    base_sample_clock: u64,
    context: &SynthVoiceFrameContext,
    samples: &mut [f32],
    block: SynthVoiceBlockRenderState,
) -> usize {
    let frames = SourceFrames {
        frames,
        base_sample_clock,
        context,
        block,
    };
    if let Some(ramp) = context.level_ramp {
        return render_level_ramp_frames(&frames, voice, ring, samples, ramp);
    }
    let gain = context.render_config.amp_gain;
    match context.render_config.source {
        VoiceSource::Synth => {
            let cfg = context.render_config;
            frames.render(
                voice,
                ring,
                samples,
                None,
                |v, _, _| synth_source_sample(&cfg, v),
                |_| gain,
            )
        }
        VoiceSource::Fm {
            index, index_env, ..
        } => frames.render(
            voice,
            ring,
            samples,
            Some(index_env),
            |v, _, _| fm_source_sample(index, v),
            |_| gain,
        ),
        VoiceSource::Pluck { .. } => frames.render(
            voice,
            ring,
            samples,
            None,
            |v, r, _| pluck_source_sample(v, r),
            |_| gain,
        ),
        VoiceSource::Drum => frames.render(
            voice,
            ring,
            samples,
            None,
            |v, _, _| v.drum.next(),
            |_| gain,
        ),
        VoiceSource::Disabled => 0,
    }
}

/// The same loops while a level glide runs, reading each level per sample along the ramp.
#[inline(never)]
fn render_level_ramp_frames(
    frames: &SourceFrames<'_>,
    voice: &mut Voice,
    ring: &mut PluckRing,
    samples: &mut [f32],
    ramp: LevelRamp,
) -> usize {
    let gain = |frame| ramp.at(2, frame);
    match frames.context.render_config.source {
        VoiceSource::Synth => {
            let mut cfg = frames.context.render_config;
            frames.render(
                voice,
                ring,
                samples,
                None,
                |v, _, frame| {
                    cfg.set_osc_levels(ramp.at(0, frame), ramp.at(1, frame));
                    synth_source_sample(&cfg, v)
                },
                gain,
            )
        }
        VoiceSource::Fm {
            index, index_env, ..
        } => frames.render(
            voice,
            ring,
            samples,
            Some(index_env),
            |v, _, _| fm_source_sample(index, v),
            gain,
        ),
        VoiceSource::Pluck { .. } => frames.render(
            voice,
            ring,
            samples,
            None,
            |v, r, _| pluck_source_sample(v, r),
            gain,
        ),
        VoiceSource::Drum => {
            frames.render(voice, ring, samples, None, |v, _, _| v.drum.next(), gain)
        }
        VoiceSource::Disabled => 0,
    }
}

struct SourceFrames<'a> {
    frames: usize,
    base_sample_clock: u64,
    context: &'a SynthVoiceFrameContext,
    block: SynthVoiceBlockRenderState,
}

impl SourceFrames<'_> {
    #[inline(always)]
    fn render(
        &self,
        voice: &mut Voice,
        ring: &mut PluckRing,
        samples: &mut [f32],
        index_env: Option<EnvConfig>,
        mut source: impl FnMut(&mut Voice, &mut PluckRing, usize) -> f32,
        amp_gain: impl Fn(usize) -> f32,
    ) -> usize {
        let context = self.context;
        let mut rendered_frames = 0;
        for (frame, sample) in samples[..self.frames].iter_mut().enumerate() {
            if self.base_sample_clock.saturating_add(frame as u64) >= voice.note_off_sample {
                voice
                    .amp_env
                    .begin_release(context.config.amp_env, context.sample_rate);
                voice
                    .filt_env
                    .begin_release(context.config.filter_env, context.sample_rate);
                if let Some(index_env) = index_env {
                    voice
                        .index_env
                        .begin_release(index_env, context.sample_rate);
                }
            }
            let amp_env = voice.amp_env.next();
            let filt_env = voice.filt_env.next();
            if voice.amp_env.is_off() {
                voice.active = false;
                break;
            }
            let dry = source(voice, ring, frame);
            *sample = shape_synth_voice_block_sample(
                context,
                voice,
                dry,
                amp_env,
                filt_env,
                self.block,
                amp_gain(frame),
            );
            rendered_frames += 1;
        }
        rendered_frames
    }
}

fn voice_source_matches(voice: &Voice, config: &SynthVoiceRenderConfig) -> bool {
    matches!(config.source, VoiceSource::Fm { .. }) == voice.fm
        && matches!(config.source, VoiceSource::Pluck { .. }) == voice.pluck_voice
        && matches!(config.source, VoiceSource::Drum) == voice.drum_voice
        && !matches!(config.source, VoiceSource::Disabled)
        && voice.source_generation == config.source_generation
}
