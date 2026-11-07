use super::sample_voice_loop::{SampleCursor, SampleVoiceLoop};
use super::support::{SampleBufferView, SampleVoice};

#[derive(Clone, Copy)]
pub(super) struct SampleSourceContext {
    pub(super) sample_rate: u32,
}

/// The frame path, which picks the voice's cursor every frame; the block paths pick it once.
pub(super) fn render_sample_voice_frame(voice: &mut SampleVoice, sample_rate: u32) -> Option<f32> {
    match voice.cursor {
        SampleVoiceLoop::Once(cursor) => render_sample_voice_frame_once(voice, cursor, sample_rate),
        SampleVoiceLoop::Forward(cursor) => {
            render_sample_voice_frame_once(voice, cursor, sample_rate)
        }
        SampleVoiceLoop::PingPong(cursor) => {
            render_sample_voice_frame_once(voice, cursor, sample_rate)
        }
    }
}

fn render_sample_voice_frame_once(
    voice: &mut SampleVoice,
    mut cursor: impl SampleCursor,
    sample_rate: u32,
) -> Option<f32> {
    let sample = render_sample_voice_frame_with(voice, &mut cursor, sample_rate);
    voice.cursor = cursor.into_loop();
    sample
}

#[inline(always)]
fn render_sample_voice_frame_with(
    voice: &mut SampleVoice,
    cursor: &mut impl SampleCursor,
    sample_rate: u32,
) -> Option<f32> {
    if !voice.active {
        return None;
    }
    let Some(buffer) = voice.buffer.as_ref() else {
        voice.active = false;
        return None;
    };
    if buffer.samples.len() / (buffer.channels as usize) == 0 {
        voice.active = false;
        return None;
    }
    let Some(sample) = cursor.read(buffer, &mut voice.pos, voice.step) else {
        voice.active = false;
        return None;
    };
    let q = sample_filter_q(voice.filter_resonance);
    let amp = if voice.shape.is_static() {
        let cutoff = voice
            .shape
            .static_cutoff(voice.filter_cutoff_hz, sample_rate);
        voice
            .filt
            .prepare(voice.shape.filter_kind, cutoff, q, sample_rate);
        1.0
    } else if let Some(amp) =
        voice
            .shape
            .next_frame(&mut voice.filt, voice.filter_cutoff_hz, q, sample_rate)
    {
        amp
    } else {
        voice.active = false;
        return None;
    };
    let filtered = voice.filt.process_prepared(sample).clamp(-8.0, 8.0);
    Some(filtered * voice.gain * amp)
}

pub(super) fn render_sample_voice_block(
    voice: &mut SampleVoice,
    frames: usize,
    sample_rate: u32,
    samples: &mut [f32],
) -> usize {
    let lead = if voice.start_delay > 0 && voice.active {
        let lead = (voice.start_delay as usize).min(frames);
        voice.start_delay -= lead as u32;
        samples[..lead].fill(0.0);
        lead
    } else {
        0
    };
    let release = voice.shape.release_frame_in_block(frames);
    let split = release.map_or(frames, |release| release.max(lead));
    let mut rendered =
        lead + render_sample_source_block(voice, split - lead, sample_rate, &mut samples[lead..]);
    if release.is_some() {
        voice.shape.begin_pending_release(sample_rate);
        rendered +=
            render_sample_source_block(voice, frames - split, sample_rate, &mut samples[split..]);
    }
    let Some(faded) = voice.declick.finish_block(
        &mut voice.last_output,
        &mut samples[..rendered],
        sample_rate,
    ) else {
        return rendered;
    };
    voice.active = false;
    faded
}

fn render_sample_source_block(
    voice: &mut SampleVoice,
    frames: usize,
    sample_rate: u32,
    samples: &mut [f32],
) -> usize {
    if frames == 0 || !voice.active {
        return 0;
    }
    let Some(buffer) = voice.buffer.as_ref() else {
        voice.active = false;
        return 0;
    };
    let buffer_frames = buffer.samples.len() / buffer.channels as usize;
    if buffer_frames == 0 || voice.pos >= buffer_frames as f32 {
        voice.active = false;
        return 0;
    }
    match voice.cursor {
        SampleVoiceLoop::Once(cursor) => {
            render_source_block_with(voice, cursor, frames, sample_rate, samples)
        }
        SampleVoiceLoop::Forward(cursor) => {
            render_source_block_with(voice, cursor, frames, sample_rate, samples)
        }
        SampleVoiceLoop::PingPong(cursor) => {
            render_source_block_with(voice, cursor, frames, sample_rate, samples)
        }
    }
}

#[inline(always)]
fn render_source_block_with(
    voice: &mut SampleVoice,
    mut cursor: impl SampleCursor,
    frames: usize,
    sample_rate: u32,
    samples: &mut [f32],
) -> usize {
    if !voice.shape.is_static() {
        let rendered =
            render_shaped_sample_voice_block(voice, &mut cursor, frames, sample_rate, samples);
        voice.cursor = cursor.into_loop();
        return rendered;
    }
    let Some(buffer) = voice.buffer.as_ref() else {
        return 0;
    };
    let buffer_view = SampleBufferView::from_buffer(buffer);
    let q = sample_filter_q(voice.filter_resonance);
    let cutoff = voice
        .shape
        .static_cutoff(voice.filter_cutoff_hz, sample_rate);
    voice
        .filt
        .prepare(voice.shape.filter_kind, cutoff, q, sample_rate);
    let read = PlainSampleRead {
        view: &buffer_view,
        step: voice.step,
        gain: voice.gain,
    };
    let (rendered_frames, ended) = if voice.filt.is_open() {
        read.frames(
            &mut cursor,
            &mut voice.pos,
            &mut samples[..frames],
            |input| input,
        )
    } else {
        let filt = &mut voice.filt;
        read.frames(
            &mut cursor,
            &mut voice.pos,
            &mut samples[..frames],
            |input| filt.process_prepared(input),
        )
    };
    voice.cursor = cursor.into_loop();
    if ended {
        voice.active = false;
    }
    rendered_frames
}

/// One plain block of sample reading; the filter is a closure so an open filter compiles to its
/// own loop with no filter work at all.
struct PlainSampleRead<'a> {
    view: &'a SampleBufferView<'a>,
    step: f32,
    gain: f32,
}

impl PlainSampleRead<'_> {
    #[inline(always)]
    fn frames(
        &self,
        cursor: &mut impl SampleCursor,
        pos: &mut f32,
        samples: &mut [f32],
        mut filter: impl FnMut(f32) -> f32,
    ) -> (usize, bool) {
        let mut rendered_frames = 0;
        for sample in samples.iter_mut() {
            let Some(input) = cursor.read(self.view, pos, self.step) else {
                return (rendered_frames, true);
            };
            let filtered = filter(input).clamp(-8.0, 8.0);
            *sample = filtered * self.gain;
            rendered_frames += 1;
        }
        (rendered_frames, false)
    }
}

/// The enveloped variant of the block loop, kept separate so plain voices stay on the plain loop;
/// it ends the voice when the amp envelope's release reaches zero.
fn render_shaped_sample_voice_block(
    voice: &mut SampleVoice,
    cursor: &mut impl SampleCursor,
    frames: usize,
    sample_rate: u32,
    samples: &mut [f32],
) -> usize {
    let mut rendered_frames = 0;
    for sample in samples[..frames].iter_mut() {
        let Some(value) = render_sample_voice_frame_with(voice, cursor, sample_rate) else {
            break;
        };
        *sample = value;
        rendered_frames += 1;
        if !voice.active {
            break;
        }
    }
    rendered_frames
}

fn sample_filter_q(resonance: f32) -> f32 {
    0.5 + (resonance.clamp(0.0, 100.0) / 100.0) * 11.5
}
