//! Where a sample voice reads next: once to the end, round a forward loop that crossfades its
//! seam, or back and forth between the loop points. A voice picks its cursor when it starts, and
//! each block runs the read loop built for that cursor.

use super::super::types::{SampleLoopMode, SamplePlaybackConfig};
use super::sample_voice::SampleBufferView;
use crate::synth::SampleBuffer;

/// The forward loop's seam crossfade, in seconds of the sample itself.
const LOOP_CROSSFADE_SECONDS: f64 = 0.010;
/// With less than this much sample before Loop Start, a forward loop's seam dips out and back
/// in over this long instead of crossfading.
const LOOP_DIP_SECONDS: f64 = 0.001;

/// How a cursor reads the input at a position. A block picks one for its whole read loop.
pub(super) trait SampleInterpolation {
    fn at(source: &SampleBufferView, pos: f64) -> f32;
}

/// Playback at the sample's own rate from a whole frame stays on whole frames, so it reads them
/// as they are.
pub(super) struct WholeFrames;

impl SampleInterpolation for WholeFrames {
    #[inline(always)]
    fn at(source: &SampleBufferView, pos: f64) -> f32 {
        source.mono_frame(pos as usize)
    }
}

/// 4-point, 3rd-order Hermite between frames; the neighbours clamp to the sample's first and
/// last frame. It passes exactly through every frame.
pub(super) struct Hermite;

impl SampleInterpolation for Hermite {
    #[inline(always)]
    fn at(source: &SampleBufferView, pos: f64) -> f32 {
        let frame = pos as usize;
        let t = (pos - frame as f64) as f32;
        let last = source.frames() - 1;
        let before = source.mono_frame(frame.saturating_sub(1));
        let x0 = source.mono_frame(frame);
        let x1 = source.mono_frame((frame + 1).min(last));
        let x2 = source.mono_frame((frame + 2).min(last));
        let c1 = 0.5 * (x1 - before);
        let c2 = before - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
        let c3 = 0.5 * (x2 - before) + 1.5 * (x0 - x1);
        ((c3 * t + c2) * t + c1) * t + x0
    }
}

/// Reads the input at `pos` and moves `pos` on by `step`; `None` once a one-shot read has run
/// off the end of the sample.
pub(super) trait SampleCursor: Copy {
    fn read<I: SampleInterpolation>(
        &mut self,
        source: &SampleBufferView,
        pos: &mut f64,
        step: f64,
    ) -> Option<f32>;
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
            let seconds = |seconds: f64| (seconds * f64::from(buffer.sample_rate)).round();
            let dips = loop_start < seconds(LOOP_DIP_SECONDS);
            let fade = if loop_frames < 4.0 {
                0.0
            } else if dips {
                seconds(LOOP_DIP_SECONDS)
            } else {
                seconds(LOOP_CROSSFADE_SECONDS).min(loop_start)
            }
            .min((loop_frames / 2.0).floor());
            Self::Forward(ForwardLoop {
                start: loop_start,
                end: loop_end,
                fade_start: loop_end - fade,
                inverse_fade: if fade > 0.0 { (1.0 / fade) as f32 } else { 0.0 },
                partner_level: if dips { 0.0 } else { 1.0 },
                fade_in_end: f64::NEG_INFINITY,
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
    fn read<I: SampleInterpolation>(
        &mut self,
        source: &SampleBufferView,
        pos: &mut f64,
        step: f64,
    ) -> Option<f32> {
        if *pos >= source.end_position() {
            return None;
        }
        let input = I::at(source, *pos);
        *pos += step;
        Some(input)
    }

    fn into_loop(self) -> SampleVoiceLoop {
        SampleVoiceLoop::Once(self)
    }
}

/// Loops forward, every pass lasting exactly the loop. Over the last `fade` frames before Loop
/// End it blends into the frames just before Loop Start, so the jump back lands where the blend
/// ends and the seam never steps. With too little sample before Loop Start for that, the seam
/// dips instead: those last frames fade out and, after each jump back, the first ones fade in.
#[derive(Clone, Copy, Debug)]
pub(super) struct ForwardLoop {
    start: f64,
    end: f64,
    fade_start: f64,
    inverse_fade: f32,
    /// 1 when the seam crossfades into the frames before Loop Start, 0 when it dips.
    partner_level: f32,
    /// Where a dipping loop's fade back in ends; before its first jump back, nowhere.
    fade_in_end: f64,
}

impl SampleCursor for ForwardLoop {
    #[inline(always)]
    fn read<I: SampleInterpolation>(
        &mut self,
        source: &SampleBufferView,
        pos: &mut f64,
        step: f64,
    ) -> Option<f32> {
        let at = *pos;
        let length = self.end - self.start;
        let mut input = I::at(source, at);
        if at >= self.fade_start {
            let blend = (at - self.fade_start) as f32 * self.inverse_fade;
            let partner = I::at(source, (at - length).max(0.0)) * self.partner_level;
            input += (partner - input) * blend;
        } else if at < self.fade_in_end {
            input *= (at - self.start) as f32 * self.inverse_fade;
        }
        let next = at + step;
        *pos = if next >= self.end {
            if self.partner_level == 0.0 {
                self.fade_in_end = self.start + (self.end - self.fade_start);
            }
            self.start + (next - self.end) % length
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
    fn read<I: SampleInterpolation>(
        &mut self,
        source: &SampleBufferView,
        pos: &mut f64,
        step: f64,
    ) -> Option<f32> {
        let input = I::at(source, *pos);
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

#[cfg(test)]
mod tests;
