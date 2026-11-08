//! Where a sample voice reads next: once to the end, round a forward loop that crossfades its
//! seam, or back and forth between the loop points. A voice picks its cursor when it starts, and
//! each block runs the read loop built for that cursor.

use super::super::types::{SampleLoopMode, SamplePlaybackConfig};
use super::sample_voice::SampleBufferView;
use crate::synth::SampleBuffer;

/// The forward loop's seam crossfade, in seconds of the sample itself.
const LOOP_CROSSFADE_SECONDS: f64 = 0.010;

#[inline(always)]
fn lerp_at(source: &SampleBufferView, pos: f64) -> f32 {
    let frame = pos as usize;
    let frac = (pos - frame as f64) as f32;
    let next_frame = (frame + 1).min(source.frames() - 1);
    source.mono_frame(frame) * (1.0 - frac) + source.mono_frame(next_frame) * frac
}

/// Reads the input at `pos` and moves `pos` on by `step`; `None` once a one-shot read has run
/// off the end of the sample.
pub(super) trait SampleCursor: Copy {
    fn read(&mut self, source: &SampleBufferView, pos: &mut f64, step: f64) -> Option<f32>;
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
    pub(super) fn start(playback: &SamplePlaybackConfig, buffer: &SampleBuffer) -> (Self, f64) {
        let frames = buffer.samples.len() / usize::from(buffer.channels.max(1));
        let at = |pct: f32| (frames as f64 * f64::from(pct) / 100.0).floor();
        let start = at(playback.start_pct);
        let loop_start = at(playback.loop_start_pct);
        let loop_end = at(playback.loop_end_pct);
        let loop_frames = loop_end - loop_start;
        if playback.loop_mode == SampleLoopMode::Off || loop_frames < 2.0 {
            return (Self::OFF, start);
        }
        let start = if start >= loop_end { loop_start } else { start };
        let cursor = if playback.loop_mode == SampleLoopMode::Forward {
            let fade = (LOOP_CROSSFADE_SECONDS * f64::from(buffer.sample_rate))
                .round()
                .min((loop_frames / 2.0).floor());
            Self::Forward(ForwardLoop {
                end: loop_end,
                fade_start: loop_end - fade,
                inverse_fade: if fade > 0.0 { (1.0 / fade) as f32 } else { 0.0 },
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
    fn read(&mut self, source: &SampleBufferView, pos: &mut f64, step: f64) -> Option<f32> {
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
    end: f64,
    fade_start: f64,
    inverse_fade: f32,
    jump: f64,
}

impl SampleCursor for ForwardLoop {
    #[inline(always)]
    fn read(&mut self, source: &SampleBufferView, pos: &mut f64, step: f64) -> Option<f32> {
        let at = *pos;
        let mut input = lerp_at(source, at);
        if at >= self.fade_start {
            let blend = (at - self.fade_start) as f32 * self.inverse_fade;
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
    start: f64,
    end: f64,
    reverse: bool,
}

impl SampleCursor for PingPongLoop {
    #[inline(always)]
    fn read(&mut self, source: &SampleBufferView, pos: &mut f64, step: f64) -> Option<f32> {
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
