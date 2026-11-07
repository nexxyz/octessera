//! Where a sample voice reads next: once to the end, round a forward loop that crossfades its
//! seam, or back and forth between the loop points. A voice picks its cursor when it starts, and
//! each block runs the read loop built for that cursor.

use super::super::types::{SampleLoopMode, SamplePlaybackConfig};
use super::sample_voice::{mono_frame, SampleBufferView};
use crate::synth::SampleBuffer;

/// The forward loop's seam crossfade, in seconds of the sample itself.
const LOOP_CROSSFADE_SECONDS: f32 = 0.010;

/// A sample's frames, as the block path's borrowed view or the frame path's buffer.
pub(super) trait MonoFrames {
    fn mono(&self, frame: usize) -> f32;
    fn last_frame(&self) -> usize;
    fn end_position(&self) -> f32;
}

impl MonoFrames for SampleBufferView<'_> {
    #[inline(always)]
    fn mono(&self, frame: usize) -> f32 {
        self.mono_frame(frame)
    }

    #[inline(always)]
    fn last_frame(&self) -> usize {
        self.frames() - 1
    }

    #[inline(always)]
    fn end_position(&self) -> f32 {
        SampleBufferView::end_position(self)
    }
}

impl MonoFrames for SampleBuffer {
    #[inline(always)]
    fn mono(&self, frame: usize) -> f32 {
        mono_frame(self, frame)
    }

    #[inline(always)]
    fn last_frame(&self) -> usize {
        self.samples.len() / self.channels as usize - 1
    }

    #[inline(always)]
    fn end_position(&self) -> f32 {
        (self.samples.len() / self.channels as usize) as f32
    }
}

#[inline(always)]
fn lerp_at(source: &impl MonoFrames, pos: f32) -> f32 {
    let frame = pos.floor() as usize;
    let frac = pos - frame as f32;
    let next_frame = (frame + 1).min(source.last_frame());
    source.mono(frame) * (1.0 - frac) + source.mono(next_frame) * frac
}

/// Reads the input at `pos` and moves `pos` on by `step`; `None` once a one-shot read has run
/// off the end of the sample.
pub(super) trait SampleCursor: Copy {
    fn read(&mut self, source: &impl MonoFrames, pos: &mut f32, step: f32) -> Option<f32>;
    fn into_loop(self) -> SampleVoiceLoop;
}

#[derive(Clone, Copy, Debug)]
pub(super) enum SampleVoiceLoop {
    Once(OnceThrough),
    Forward(ForwardLoop),
    PingPong(PingPongLoop),
}

impl SampleVoiceLoop {
    pub(super) const OFF: Self = Self::Once(OnceThrough);

    /// The cursor for a voice and the frame it starts on. A loop that would be shorter than two
    /// frames plays once instead; a looping start past Loop End begins at Loop Start.
    pub(super) fn start(playback: &SamplePlaybackConfig, buffer: &SampleBuffer) -> (Self, f32) {
        let frames = buffer.samples.len() / usize::from(buffer.channels.max(1));
        let at = |pct: f32| (frames as f32 * pct / 100.0).floor();
        let start = at(playback.start_pct);
        let loop_start = at(playback.loop_start_pct);
        let loop_end = at(playback.loop_end_pct);
        let loop_frames = loop_end - loop_start;
        if playback.loop_mode == SampleLoopMode::Off || loop_frames < 2.0 {
            return (Self::OFF, start);
        }
        let start = if start >= loop_end { loop_start } else { start };
        let cursor = if playback.loop_mode == SampleLoopMode::Forward {
            let fade = (LOOP_CROSSFADE_SECONDS * buffer.sample_rate as f32)
                .round()
                .min((loop_frames / 2.0).floor());
            Self::Forward(ForwardLoop {
                end: loop_end,
                fade_start: loop_end - fade,
                inverse_fade: if fade > 0.0 { 1.0 / fade } else { 0.0 },
                jump: loop_frames - fade,
            })
        } else {
            Self::PingPong(PingPongLoop {
                start: loop_start,
                end: loop_end - 1.0,
                reverse: false,
            })
        };
        (cursor, start)
    }
}

/// Plays to the end of the sample, then stops.
#[derive(Clone, Copy, Debug)]
pub(super) struct OnceThrough;

impl SampleCursor for OnceThrough {
    #[inline(always)]
    fn read(&mut self, source: &impl MonoFrames, pos: &mut f32, step: f32) -> Option<f32> {
        if *pos >= source.end_position() {
            return None;
        }
        let input = lerp_at(source, *pos);
        *pos += step;
        Some(input)
    }

    fn into_loop(self) -> SampleVoiceLoop {
        SampleVoiceLoop::Once(self)
    }
}

/// Loops forward: over the last `fade` frames before Loop End it blends into the frames after
/// Loop Start, then jumps on past them, so the seam never steps.
#[derive(Clone, Copy, Debug)]
pub(super) struct ForwardLoop {
    end: f32,
    fade_start: f32,
    inverse_fade: f32,
    jump: f32,
}

impl SampleCursor for ForwardLoop {
    #[inline(always)]
    fn read(&mut self, source: &impl MonoFrames, pos: &mut f32, step: f32) -> Option<f32> {
        let at = *pos;
        let mut input = lerp_at(source, at);
        if at >= self.fade_start {
            let blend = (at - self.fade_start) * self.inverse_fade;
            input += (lerp_at(source, at - self.jump) - input) * blend;
        }
        let next = at + step;
        *pos = if next >= self.end {
            self.end - self.jump + (next - self.end) % self.jump
        } else {
            next
        };
        Some(input)
    }

    fn into_loop(self) -> SampleVoiceLoop {
        SampleVoiceLoop::Forward(self)
    }
}

/// Bounces between Loop Start and the last frame before Loop End; the turn mirrors the read
/// position, so the waveform stays continuous.
#[derive(Clone, Copy, Debug)]
pub(super) struct PingPongLoop {
    start: f32,
    end: f32,
    reverse: bool,
}

impl SampleCursor for PingPongLoop {
    #[inline(always)]
    fn read(&mut self, source: &impl MonoFrames, pos: &mut f32, step: f32) -> Option<f32> {
        let input = lerp_at(source, *pos);
        if self.reverse {
            let next = *pos - step;
            if next < self.start {
                *pos = (2.0 * self.start - next).min(self.end);
                self.reverse = false;
            } else {
                *pos = next;
            }
        } else {
            let next = *pos + step;
            if next > self.end {
                *pos = (2.0 * self.end - next).max(self.start);
                self.reverse = true;
            } else {
                *pos = next;
            }
        }
        Some(input)
    }

    fn into_loop(self) -> SampleVoiceLoop {
        SampleVoiceLoop::PingPong(self)
    }
}
