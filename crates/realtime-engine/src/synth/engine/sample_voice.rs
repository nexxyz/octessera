use super::*;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static SAMPLE_BUFFER_VIEW_RESOLVES: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Copy)]
pub(super) struct SampleBufferView<'a> {
    samples: &'a [f32],
    channels: usize,
    frames: usize,
    end_position: f32,
}

impl<'a> SampleBufferView<'a> {
    pub(super) fn from_buffer(buffer: &'a SampleBuffer) -> Self {
        #[cfg(test)]
        SAMPLE_BUFFER_VIEW_RESOLVES.with(|resolves| resolves.set(resolves.get() + 1));
        let channels = buffer.channels as usize;
        let frames = buffer.samples.len() / channels;
        Self {
            samples: buffer.samples.as_ref(),
            channels,
            frames,
            end_position: frames as f32,
        }
    }

    pub(super) fn frames(&self) -> usize {
        self.frames
    }

    pub(super) fn end_position(&self) -> f32 {
        self.end_position
    }

    pub(super) fn mono_frame(&self, frame: usize) -> f32 {
        let base = frame.saturating_mul(self.channels);
        if self.channels == 1 {
            return self.samples.get(base).copied().unwrap_or(0.0);
        }
        let left = self.samples.get(base).copied().unwrap_or(0.0);
        let right = self.samples.get(base + 1).copied().unwrap_or(left);
        (left + right) * 0.5
    }
}

#[cfg(test)]
pub(super) fn reset_sample_buffer_view_resolves_for_test() {
    SAMPLE_BUFFER_VIEW_RESOLVES.with(|resolves| resolves.set(0));
}

#[cfg(test)]
pub(super) fn sample_buffer_view_resolves_for_test() -> usize {
    SAMPLE_BUFFER_VIEW_RESOLVES.with(Cell::get)
}

#[derive(Clone, Debug)]
pub(super) struct SampleVoice {
    pub(super) active: bool,
    pub(super) instrument_slot: u8,
    pub(super) sample_slot: usize,
    pub(super) tile: Option<u8>,
    pub(super) buffer: Option<SampleBuffer>,
    pub(super) filter_cutoff_hz: f32,
    pub(super) filter_resonance: f32,
    pub(super) pos: f32,
    pub(super) step: f32,
    /// Where the next read goes: on to the end, or round the slot's loop.
    pub(super) cursor: super::sample_voice_loop::SampleVoiceLoop,
    pub(super) gain: f32,
    pub(super) filt: BiquadState,
    pub(super) shape: super::sample_voice_shape::SampleVoiceShape,
    pub(super) last_output: f32,
    pub(super) declick: StealDeclick,
    /// Silent frames left before the voice starts, for an event placed inside a block.
    pub(super) start_delay: u32,
}

impl SampleVoice {
    pub(super) fn off() -> Self {
        Self {
            active: false,
            instrument_slot: 0,
            sample_slot: 0,
            tile: None,
            buffer: None,
            filter_cutoff_hz: 8000.0,
            filter_resonance: 20.0,
            pos: 0.0,
            step: 1.0,
            cursor: super::sample_voice_loop::SampleVoiceLoop::OFF,
            gain: 0.0,
            filt: BiquadState::new(),
            shape: super::sample_voice_shape::SampleVoiceShape::off(),
            last_output: 0.0,
            declick: StealDeclick::default(),
            start_delay: 0,
        }
    }
}

#[derive(Clone, Debug)]
/// A browser audition: the file at its own pitch and level, mixed straight into the master.
pub(super) struct PreviewSampleVoice {
    pub(super) buffer: SampleBuffer,
    pub(super) pos: f32,
    pub(super) step: f32,
}

pub(super) fn sample_slot_for_note(note: u8) -> usize {
    note.saturating_sub(36)
        .min((SAMPLE_SLOTS_PER_INSTRUMENT - 1) as u8) as usize
}
