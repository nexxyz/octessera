use super::support::{mono_frame, SampleBufferView, SampleVoice};

#[derive(Clone, Copy)]
pub(super) struct SampleSourceContext {
    pub(super) sample_rate: u32,
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
    voice.pos += voice.step;
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
    let rendered =
        lead + render_sample_source_block(voice, frames - lead, sample_rate, &mut samples[lead..]);
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
    let buffer_view = SampleBufferView::from_buffer(buffer);
    let buffer_frames = buffer_view.frames();
    let end_position = buffer_view.end_position();
    if buffer_frames == 0 || voice.pos >= end_position {
        voice.active = false;
        return 0;
    }
    if !voice.shape.is_static() {
        return render_shaped_sample_voice_block(voice, frames, sample_rate, samples);
    }
    let q = sample_filter_q(voice.filter_resonance);
    let cutoff = voice
        .shape
        .static_cutoff(voice.filter_cutoff_hz, sample_rate);
    voice
        .filt
        .prepare(voice.shape.filter_kind, cutoff, q, sample_rate);
    let read = PlainSampleRead {
        view: &buffer_view,
        buffer_frames,
        end_position,
        step: voice.step,
        gain: voice.gain,
    };
    let (rendered_frames, ended) = if voice.filt.is_open() {
        read.frames(&mut voice.pos, &mut samples[..frames], |input| input)
    } else {
        let filt = &mut voice.filt;
        read.frames(&mut voice.pos, &mut samples[..frames], |input| {
            filt.process_prepared(input)
        })
    };
    if ended {
        voice.active = false;
    }
    rendered_frames
}

/// One plain block of sample reading; the filter is a closure so an open filter compiles to its
/// own loop with no filter work at all.
struct PlainSampleRead<'a> {
    view: &'a SampleBufferView<'a>,
    buffer_frames: usize,
    end_position: f32,
    step: f32,
    gain: f32,
}

impl PlainSampleRead<'_> {
    #[inline(always)]
    fn frames(
        &self,
        pos: &mut f32,
        samples: &mut [f32],
        mut filter: impl FnMut(f32) -> f32,
    ) -> (usize, bool) {
        let mut rendered_frames = 0;
        for sample in samples.iter_mut() {
            if *pos >= self.end_position {
                return (rendered_frames, true);
            }
            let frame = pos.floor() as usize;
            let frac = *pos - frame as f32;
            let next_frame = (frame + 1).min(self.buffer_frames - 1);
            let input = self.view.mono_frame(frame) * (1.0 - frac)
                + self.view.mono_frame(next_frame) * frac;
            let filtered = filter(input).clamp(-8.0, 8.0);
            *pos += self.step;
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
    frames: usize,
    sample_rate: u32,
    samples: &mut [f32],
) -> usize {
    let mut rendered_frames = 0;
    for sample in samples[..frames].iter_mut() {
        let Some(value) = render_sample_voice_frame(voice, sample_rate) else {
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
