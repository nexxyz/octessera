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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InstrumentKind {
    Synth,
    Fm,
    Pluck,
    Drum,
    Sample,
    Midi,
    None,
}

#[derive(Clone, Debug)]
pub(super) struct SampleVoice {
    pub(super) active: bool,
    pub(super) canonical_lane: Option<super::super::types::LogicalLaneId>,
    pub(super) instrument_slot: u8,
    pub(super) sample_slot: usize,
    pub(super) buffer: Option<SampleBuffer>,
    pub(super) filter_cutoff_hz: f32,
    pub(super) filter_resonance: f32,
    pub(super) pos: f32,
    pub(super) step: f32,
    pub(super) gain: f32,
    pub(super) filt: BiquadState,
}

impl SampleVoice {
    pub(super) fn off() -> Self {
        Self {
            active: false,
            canonical_lane: None,
            instrument_slot: 0,
            sample_slot: 0,
            buffer: None,
            filter_cutoff_hz: 8000.0,
            filter_resonance: 20.0,
            pos: 0.0,
            step: 1.0,
            gain: 0.0,
            filt: BiquadState::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct PreviewSampleVoice {
    pub(super) slot: usize,
    pub(super) buffer: SampleBuffer,
    pub(super) pos: f32,
    pub(super) step: f32,
    pub(super) gain: f32,
    pub(super) filt: BiquadState,
}

pub(super) fn parse_route(route: &str) -> usize {
    if route == "direct" {
        return 0;
    }
    if let Some(rest) = route.strip_prefix("fx_bus_") {
        if let Ok(n) = rest.parse::<usize>() {
            if n >= 1 {
                return n;
            }
        }
    }
    0
}

pub(super) fn parse_instrument_kind(kind: &str) -> InstrumentKind {
    match kind {
        "synth" => InstrumentKind::Synth,
        "fm" => InstrumentKind::Fm,
        "pluck" => InstrumentKind::Pluck,
        "drum" => InstrumentKind::Drum,
        "sampler" => InstrumentKind::Sample,
        "midi" => InstrumentKind::Midi,
        "none" => InstrumentKind::None,
        _ => InstrumentKind::None,
    }
}

pub(super) fn param_f32(params: &BTreeMap<String, Value>, key: &str, fallback: f32) -> f32 {
    params
        .get(key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

pub(super) fn sample_slot_for_note(note: u8) -> usize {
    note.saturating_sub(36)
        .min((SAMPLE_SLOTS_PER_INSTRUMENT - 1) as u8) as usize
}

pub(super) fn mono_frame(buffer: &SampleBuffer, frame: usize) -> f32 {
    let channels = buffer.channels.max(1) as usize;
    let base = frame.saturating_mul(channels);
    if channels == 1 {
        return buffer.samples.get(base).copied().unwrap_or(0.0);
    }
    let left = buffer.samples.get(base).copied().unwrap_or(0.0);
    let right = buffer.samples.get(base + 1).copied().unwrap_or(left);
    (left + right) * 0.5
}

pub(super) fn pan_gains(pan_pos: usize, positions: usize) -> (f32, f32) {
    if positions <= 1 {
        return (0.70710677, 0.70710677);
    }
    let t = (pan_pos.min(positions - 1) as f32) / ((positions - 1) as f32);
    let theta = t * (std::f32::consts::FRAC_PI_2);
    (theta.cos(), theta.sin())
}

pub(super) fn pan_gains_float(pos: f32) -> (f32, f32) {
    let theta = pos.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (theta.cos(), theta.sin())
}

pub(super) fn midi_note_to_hz(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::parse_route;

    #[test]
    fn obsolete_bus_route_falls_back_to_direct() {
        assert_eq!(parse_route("bus_1"), 0);
    }
}
