use super::super::*;
use super::set_clamped_f32;
use crate::synth::engine::instrument_param_glide::InstrumentGlideParam as Glide;
use crate::synth::scalar_param::{FmParamId, ScalarMutation};

impl SynthEngine {
    pub fn set_fm_param_typed(
        &mut self,
        instrument_slot: usize,
        id: FmParamId,
        value: f32,
    ) -> ScalarMutation {
        if self.routing_tree_rejects_control_mutation() {
            return ScalarMutation::Rejected;
        }
        let slot = instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] != InstrumentKind::Fm || !value.is_finite() {
            return ScalarMutation::Rejected;
        }
        let source = &mut self.synth_render_configs[slot].source;
        let render_voice::VoiceSource::Fm {
            base_ratio,
            effective_ratio,
            ratio_fine_cents,
            index_env,
            ..
        } = source
        else {
            return ScalarMutation::Rejected;
        };
        let synth = &mut self.instruments[slot];
        let mutation = match id {
            FmParamId::RatioFineCents => {
                let mut cents = f32::from(*ratio_fine_cents);
                let mutation = set_clamped_f32(&mut cents, value.round(), -100.0, 100.0, 1.0);
                if mutation == ScalarMutation::Changed {
                    *ratio_fine_cents = cents as i16;
                    *effective_ratio = if cents == 0.0 {
                        *base_ratio
                    } else {
                        *base_ratio * 2.0_f32.powf(cents / 1200.0)
                    };
                }
                mutation
            }
            FmParamId::Index => {
                return self.glide_instrument_param(
                    slot,
                    Glide::FmIndex,
                    value.clamp(0.0, 100.0) * 0.04,
                )
            }
            FmParamId::VelocityToIndexPct => {
                return self.glide_instrument_param(slot, Glide::FmVelocityToIndex, value * 0.01)
            }
            FmParamId::ModShapePct => {
                return self.glide_instrument_param(slot, Glide::FmModShape, value * 0.01)
            }
            FmParamId::ModMixPct => {
                return self.glide_instrument_param(slot, Glide::FmModMix, value * 0.01)
            }
            FmParamId::IndexEnvAttackMs => {
                set_clamped_f32(&mut index_env.attack_ms, value, 0.0, 5000.0, 1.0)
            }
            FmParamId::IndexEnvDecayMs => {
                set_clamped_f32(&mut index_env.decay_ms, value, 0.0, 5000.0, 1.0)
            }
            FmParamId::IndexEnvSustainPct => {
                set_clamped_f32(&mut index_env.sustain_pct, value, 0.0, 100.0, 1.0)
            }
            FmParamId::IndexEnvReleaseMs => {
                set_clamped_f32(&mut index_env.release_ms, value, 0.0, 10000.0, 1.0)
            }
            FmParamId::AmpGainPct => {
                return self.glide_instrument_param(slot, Glide::AmpGain, value)
            }
            FmParamId::AmpVelocitySensitivityPct => {
                return self.glide_instrument_param(slot, Glide::AmpVelocitySensitivity, value)
            }
            FmParamId::AmpEnvAttackMs => {
                set_clamped_f32(&mut synth.amp_env.attack_ms, value, 0.0, 5000.0, 1.0)
            }
            FmParamId::AmpEnvDecayMs => {
                set_clamped_f32(&mut synth.amp_env.decay_ms, value, 0.0, 5000.0, 1.0)
            }
            FmParamId::AmpEnvSustainPct => {
                set_clamped_f32(&mut synth.amp_env.sustain_pct, value, 0.0, 100.0, 1.0)
            }
            FmParamId::AmpEnvReleaseMs => {
                set_clamped_f32(&mut synth.amp_env.release_ms, value, 0.0, 10000.0, 1.0)
            }
            FmParamId::FilterCutoffHz => {
                return self.glide_instrument_param(slot, Glide::FilterCutoff, value)
            }
            FmParamId::FilterResonance => {
                return self.glide_instrument_param(slot, Glide::FilterResonance, value)
            }
            FmParamId::FilterEnvAmountPct => {
                return self.glide_instrument_param(slot, Glide::FilterEnvAmount, value)
            }
            FmParamId::FilterKeyTrackingPct => {
                return self.glide_instrument_param(slot, Glide::FilterKeyTracking, value)
            }
            FmParamId::FilterEnvAttackMs => {
                set_clamped_f32(&mut synth.filter_env.attack_ms, value, 0.0, 5000.0, 1.0)
            }
            FmParamId::FilterEnvDecayMs => {
                set_clamped_f32(&mut synth.filter_env.decay_ms, value, 0.0, 5000.0, 1.0)
            }
            FmParamId::FilterEnvSustainPct => {
                set_clamped_f32(&mut synth.filter_env.sustain_pct, value, 0.0, 100.0, 1.0)
            }
            FmParamId::FilterEnvReleaseMs => {
                set_clamped_f32(&mut synth.filter_env.release_ms, value, 0.0, 10000.0, 1.0)
            }
        };
        if mutation == ScalarMutation::Changed {
            self.refresh_synth_render_config(slot);
        }
        mutation
    }
}
