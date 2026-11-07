use super::super::scalar_param::ScalarMutation;
use super::momentary_fx_state::{parse_momentary_fx_kind, MomentaryFxKind, MomentaryFxState};
use super::momentary_fx_voice::{prepare_voice_update, voice_param_keys};
use super::retired_state::{store_retired_momentary, RetiredAudioState};
use super::*;

#[derive(Clone)]
pub struct PreparedMomentaryFxStart {
    pub(super) state: MomentaryFxState,
}

impl PreparedMomentaryFxStart {
    pub fn epoch(&self) -> u64 {
        self.state.epoch
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreparedMomentaryFxUpdate {
    Stutter {
        epoch: u64,
        depth: f32,
        segment_len: usize,
    },
    Freeze {
        epoch: u64,
        mix: f32,
        release_len: u32,
    },
    FilterSweep {
        epoch: u64,
        target_cutoff: f32,
        q: f32,
        sweep_in_step: f32,
        sweep_out_step: f32,
    },
    PitchShift {
        epoch: u64,
        target_octaves: f32,
        mix: f32,
        slide_in_len: u32,
        slide_out_len: u32,
    },
    TapeStop {
        epoch: u64,
        stop_len: u32,
        start_len: u32,
    },
    Reverse {
        epoch: u64,
        window_len: u32,
        mix: f32,
    },
    TranceGate {
        epoch: u64,
        period: f32,
        depth: f32,
        smooth: f32,
    },
    DubThrow {
        epoch: u64,
        delay_len: f32,
        feedback: f32,
        mix: f32,
    },
    Wash {
        epoch: u64,
        feedback: f32,
        mix: f32,
        release_len: u32,
    },
    HpRiser {
        epoch: u64,
        target_octaves: f32,
        q: f32,
        sweep_in_step: f32,
        sweep_out_step: f32,
    },
    Crush {
        epoch: u64,
        bits: u32,
        rate_div: u32,
        mix: f32,
    },
}

impl PreparedMomentaryFxUpdate {
    pub fn epoch(self) -> u64 {
        match self {
            Self::Stutter { epoch, .. }
            | Self::Freeze { epoch, .. }
            | Self::FilterSweep { epoch, .. }
            | Self::PitchShift { epoch, .. }
            | Self::TapeStop { epoch, .. }
            | Self::Reverse { epoch, .. }
            | Self::TranceGate { epoch, .. }
            | Self::DubThrow { epoch, .. }
            | Self::Wash { epoch, .. }
            | Self::HpRiser { epoch, .. }
            | Self::Crush { epoch, .. } => epoch,
        }
    }
}

impl SynthEngine {
    pub fn apply_prepared_momentary_fx_start(
        &mut self,
        prepared: PreparedMomentaryFxStart,
    ) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        #[cfg(feature = "routing-tree-executor")]
        if self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_none()
        {
            store_retired_momentary(&mut retired.displaced_momentary_fx, prepared.state);
            self.reject_routing_tree_mutation_for_control();
            return retired;
        }
        let mut state = prepared.state;
        if self
            .momentary_fx
            .iter_mut()
            .any(|tail| tail.take_over_tail(&mut state))
        {
            store_retired_momentary(&mut retired.displaced_momentary_fx, state);
            return retired;
        }
        if let Some(pos) = self.momentary_fx.iter().position(|fx| fx.id == state.id) {
            store_retired_momentary(
                &mut retired.displaced_momentary_fx,
                self.momentary_fx.remove(pos),
            );
        }
        if !self.make_room_for_momentary_press(state.kind, &mut retired) {
            store_retired_momentary(&mut retired.displaced_momentary_fx, state);
            return retired;
        }
        self.momentary_fx.push(state);
        retired
    }

    /// Two Play FX run at once, one per type. A releasing tail in the way fades out for the
    /// new press; held effects in the way turn it down.
    fn make_room_for_momentary_press(
        &mut self,
        kind: MomentaryFxKind,
        retired: &mut RetiredAudioState,
    ) -> bool {
        let running = |fx: &MomentaryFxState| fx.retire_fade.is_none();
        let same_kind = self
            .momentary_fx
            .iter()
            .position(|fx| running(fx) && fx.kind == kind);
        let full = self.momentary_fx.iter().filter(|fx| running(fx)).count()
            >= super::control::MAX_MOMENTARY_FX;
        if same_kind.is_none() && !full {
            return true;
        }
        let Some(mut victim) = same_kind.or_else(|| {
            self.momentary_fx
                .iter()
                .position(|fx| running(fx) && fx.releasing)
        }) else {
            return false;
        };
        if !self.momentary_fx[victim].releasing {
            return false;
        }
        if let Some(fading) = self.momentary_fx.iter().position(|fx| !running(fx)) {
            store_retired_momentary(
                &mut retired.displaced_momentary_fx,
                self.momentary_fx.remove(fading),
            );
            if fading < victim {
                victim -= 1;
            }
        }
        self.momentary_fx[victim].begin_retire_fade(self.sample_rate);
        true
    }

    pub fn apply_prepared_momentary_fx_update(
        &mut self,
        update: PreparedMomentaryFxUpdate,
    ) -> ScalarMutation {
        #[cfg(feature = "routing-tree-executor")]
        if self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_none()
        {
            self.reject_routing_tree_mutation_for_control();
            return ScalarMutation::Rejected;
        }
        let Some(fx) = self
            .momentary_fx
            .iter_mut()
            .find(|fx| fx.epoch == update.epoch())
        else {
            return ScalarMutation::Rejected;
        };
        fx.voice.apply_update(update)
    }
}

pub fn prepare_momentary_fx_start(
    id: String,
    fx_type: String,
    params: BTreeMap<String, Value>,
    target: MomentaryFxTarget,
    sample_rate: u32,
) -> Option<PreparedMomentaryFxStart> {
    prepare_momentary_fx_start_with_epoch(id, 0, fx_type, params, target, sample_rate)
}

pub fn prepare_momentary_fx_start_with_epoch(
    id: String,
    epoch: u64,
    fx_type: String,
    params: BTreeMap<String, Value>,
    target: MomentaryFxTarget,
    sample_rate: u32,
) -> Option<PreparedMomentaryFxStart> {
    let kind = parse_momentary_fx_kind(&fx_type)?;
    validate_momentary_params(kind, &params, sample_rate)?;
    Some(PreparedMomentaryFxStart {
        state: MomentaryFxState::new_with_epoch(id, epoch, kind, &params, target, sample_rate),
    })
}

pub fn prepare_momentary_fx_update(
    epoch: u64,
    fx_type: String,
    params: BTreeMap<String, Value>,
    sample_rate: u32,
) -> Option<PreparedMomentaryFxUpdate> {
    let kind = parse_momentary_fx_kind(&fx_type)?;
    validate_momentary_params(kind, &params, sample_rate)?;
    Some(prepare_voice_update(kind, epoch, &params, sample_rate))
}

fn validate_momentary_params(
    kind: MomentaryFxKind,
    params: &BTreeMap<String, Value>,
    sample_rate: u32,
) -> Option<()> {
    if sample_rate == 0 {
        return None;
    }
    let keys = voice_param_keys(kind);
    params
        .iter()
        .all(|(key, value)| {
            keys.contains(&key.as_str())
                && value
                    .as_f64()
                    .is_some_and(|value| (value as f32).is_finite())
        })
        .then_some(())
}

#[cfg(test)]
mod momentary_control_tests;
#[cfg(test)]
mod shipped_stamp_tests;
