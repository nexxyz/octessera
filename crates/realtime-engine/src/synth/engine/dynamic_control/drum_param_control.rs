use super::super::*;
use super::set_clamped_f32;
use crate::synth::engine::instrument_param_glide::InstrumentGlideParam as Glide;
use crate::synth::scalar_param::{DrumParamId, ScalarMutation};

impl SynthEngine {
    pub fn set_drum_param_typed(
        &mut self,
        instrument_slot: usize,
        voice: u8,
        id: DrumParamId,
        value: f32,
    ) -> ScalarMutation {
        if self.routing_tree_rejects_control_mutation() {
            return ScalarMutation::Rejected;
        }
        if instrument_slot >= INSTRUMENT_SLOT_COUNT
            || voice >= 8
            || !value.is_finite()
            || self.slot_kind[instrument_slot] != InstrumentKind::Drum
            || (!id.is_voice_param() && voice != 0)
        {
            return ScalarMutation::Rejected;
        }
        let slot = instrument_slot;
        if id.is_voice_param() {
            let kit = &mut self.drum_voices[slot][usize::from(voice)];
            return match id {
                DrumParamId::TuneSemis => {
                    let next = value.round().clamp(-12.0, 12.0) as i8;
                    if kit.tune_semis == next {
                        ScalarMutation::Unchanged
                    } else {
                        kit.tune_semis = next;
                        ScalarMutation::Changed
                    }
                }
                DrumParamId::DecayMs => {
                    set_clamped_f32(&mut kit.decay_ms, value.round(), 20.0, 2_000.0, 1.0)
                }
                DrumParamId::TonePct => {
                    set_clamped_f32(&mut kit.tone_pct, value.round(), 0.0, 100.0, 1.0)
                }
                DrumParamId::AttackMs => {
                    set_clamped_f32(&mut kit.attack_ms, value.round(), 0.0, 50.0, 1.0)
                }
                DrumParamId::SweepSemis => {
                    let next = value.round().clamp(0.0, 36.0) as u8;
                    if kit.sweep_semis == next {
                        ScalarMutation::Unchanged
                    } else {
                        kit.sweep_semis = next;
                        ScalarMutation::Changed
                    }
                }
                DrumParamId::SweepMs => {
                    set_clamped_f32(&mut kit.sweep_ms, value.round(), 0.0, 200.0, 1.0)
                }
                DrumParamId::NoiseMixPct => {
                    set_clamped_f32(&mut kit.noise_mix_pct, value.round(), 0.0, 100.0, 1.0)
                }
                DrumParamId::LevelPct => {
                    set_clamped_f32(&mut kit.level_pct, value.round(), 0.0, 100.0, 1.0)
                }
                _ => ScalarMutation::Rejected,
            };
        }
        let param = match id {
            DrumParamId::AmpGainPct => Glide::AmpGain,
            DrumParamId::AmpVelocitySensitivityPct => Glide::AmpVelocitySensitivity,
            DrumParamId::FilterCutoffHz => Glide::FilterCutoff,
            DrumParamId::FilterResonance => Glide::FilterResonance,
            DrumParamId::FilterEnvAmountPct => Glide::FilterEnvAmount,
            DrumParamId::FilterKeyTrackingPct => Glide::FilterKeyTracking,
            _ => return ScalarMutation::Rejected,
        };
        self.glide_instrument_param(slot, param, value)
    }
}
