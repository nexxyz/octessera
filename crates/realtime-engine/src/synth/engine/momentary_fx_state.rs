use super::momentary_fx_voice::{GainRamp, MomentaryFxVoice, PitchStopAction, ENVELOPE_MS};
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MomentaryFxKind {
    Stutter,
    Freeze,
    FilterSweep,
    PitchShift,
    TapeStop,
    Reverse,
    TranceGate,
    DubThrow,
    Wash,
    HpRiser,
    Crush,
}

#[derive(Clone, Debug)]
pub(super) struct MomentaryFxState {
    pub(super) id: String,
    pub(super) epoch: u64,
    pub(super) kind: MomentaryFxKind,
    pub(super) target: MomentaryFxTarget,
    pub(super) releasing: bool,
    pub(super) voice: MomentaryFxVoice,
    /// Set once a new press has taken this releasing tail's place; the tail fades out over it.
    pub(super) retire_fade: Option<GainRamp>,
}

impl MomentaryFxState {
    #[cfg(test)]
    pub(super) fn new(
        id: String,
        kind: MomentaryFxKind,
        params: &BTreeMap<String, Value>,
        target: MomentaryFxTarget,
        sample_rate: u32,
    ) -> Self {
        Self::new_with_epoch(id, 0, kind, params, target, sample_rate)
    }

    pub(super) fn new_with_epoch(
        id: String,
        epoch: u64,
        kind: MomentaryFxKind,
        params: &BTreeMap<String, Value>,
        target: MomentaryFxTarget,
        sample_rate: u32,
    ) -> Self {
        Self {
            id,
            epoch,
            kind,
            target,
            releasing: false,
            voice: MomentaryFxVoice::new(kind, params, sample_rate),
            retire_fade: None,
        }
    }

    /// Lets a new press of the same type and target take over this releasing tail, so a dub
    /// throw's echoes or a wash keep ringing instead of being cut. `incoming` keeps the old id.
    pub(super) fn take_over_tail(&mut self, incoming: &mut MomentaryFxState) -> bool {
        if !self.releasing
            || self.retire_fade.is_some()
            || self.kind != incoming.kind
            || self.target != incoming.target
            || !self.voice.revive_with(&incoming.voice)
        {
            return false;
        }
        std::mem::swap(&mut self.id, &mut incoming.id);
        self.epoch = incoming.epoch;
        self.releasing = false;
        true
    }

    /// Fades a releasing tail out so a new press can take its place without a click.
    pub(super) fn begin_retire_fade(&mut self, sample_rate: u32) {
        self.retire_fade = Some(GainRamp::new(1.0, ENVELOPE_MS, sample_rate));
    }

    pub(super) fn retire_faded(&self) -> bool {
        self.retire_fade.is_some_and(|fade| fade.value() <= 0.0)
    }

    /// The release of a press: true when the effect ends at once instead of fading out.
    pub(super) fn stop(&mut self) -> bool {
        match &mut self.voice {
            MomentaryFxVoice::Stutter(_) => return true,
            MomentaryFxVoice::PitchShift(voice) => match voice.stop() {
                PitchStopAction::Retire => return true,
                PitchStopAction::Ignore => return false,
                PitchStopAction::Release => {}
            },
            _ => self.voice.release(),
        }
        self.releasing = true;
        false
    }
}

pub(in crate::synth) fn is_momentary_fx_type(kind: &str) -> bool {
    parse_momentary_fx_kind(kind).is_some()
}

pub(super) fn parse_momentary_fx_kind(kind: &str) -> Option<MomentaryFxKind> {
    match kind {
        "stutter" => Some(MomentaryFxKind::Stutter),
        "freeze" => Some(MomentaryFxKind::Freeze),
        "filter_sweep" => Some(MomentaryFxKind::FilterSweep),
        "pitch_shift" => Some(MomentaryFxKind::PitchShift),
        "tape_stop" => Some(MomentaryFxKind::TapeStop),
        "reverse" => Some(MomentaryFxKind::Reverse),
        "trance_gate" => Some(MomentaryFxKind::TranceGate),
        "dub_throw" => Some(MomentaryFxKind::DubThrow),
        "wash" => Some(MomentaryFxKind::Wash),
        "hp_riser" => Some(MomentaryFxKind::HpRiser),
        "crush" => Some(MomentaryFxKind::Crush),
        _ => None,
    }
}
