use super::super::*;
use super::set_clamped_f32;
use crate::synth::engine::instrument_param_glide::InstrumentGlideParam as Glide;
use crate::synth::scalar_param::{PluckParamId, ScalarMutation};

impl SynthEngine {
    pub fn set_pluck_param_typed(
        &mut self,
        instrument_slot: usize,
        id: PluckParamId,
        value: f32,
    ) -> ScalarMutation {
        if self.routing_tree_rejects_control_mutation() {
            return ScalarMutation::Rejected;
        }
        let slot = instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] != InstrumentKind::Pluck || !value.is_finite() {
            return ScalarMutation::Rejected;
        }
        let synth = &mut self.instruments[slot];
        let source = &mut self.synth_render_configs[slot].source;
        let render_voice::VoiceSource::Pluck { settings } = source else {
            return ScalarMutation::Rejected;
        };
        let mutation = match id {
            PluckParamId::DecayMs => {
                return self.glide_instrument_param(slot, Glide::PluckDecay, value)
            }
            PluckParamId::BrightnessPct => {
                return self.glide_instrument_param(slot, Glide::PluckBrightness, value)
            }
            PluckParamId::PickPositionPct => set_clamped_f32(
                &mut settings.pick_position_pct,
                value.round(),
                5.0,
                50.0,
                1.0,
            ),
            PluckParamId::PickDepthPct => {
                set_clamped_f32(&mut settings.pick_depth_pct, value.round(), 0.0, 100.0, 1.0)
            }
            PluckParamId::DispersionPct => {
                set_clamped_f32(&mut settings.dispersion_pct, value.round(), 0.0, 100.0, 1.0)
            }
            PluckParamId::BodyAmountPct => set_clamped_f32(
                &mut settings.body_amount_pct,
                value.round(),
                0.0,
                100.0,
                1.0,
            ),
            PluckParamId::BodyFrequencyHz => set_clamped_f32(
                &mut settings.body_frequency_hz,
                value.round(),
                100.0,
                2000.0,
                1.0,
            ),
            PluckParamId::AmpGainPct => {
                return self.glide_instrument_param(slot, Glide::AmpGain, value)
            }
            PluckParamId::AmpVelocitySensitivityPct => {
                return self.glide_instrument_param(slot, Glide::AmpVelocitySensitivity, value)
            }
            PluckParamId::AmpEnvAttackMs => {
                set_clamped_f32(&mut synth.amp_env.attack_ms, value, 0.0, 5000.0, 1.0)
            }
            PluckParamId::AmpEnvDecayMs => {
                set_clamped_f32(&mut synth.amp_env.decay_ms, value, 0.0, 5000.0, 1.0)
            }
            PluckParamId::AmpEnvSustainPct => {
                set_clamped_f32(&mut synth.amp_env.sustain_pct, value, 0.0, 100.0, 1.0)
            }
            PluckParamId::AmpEnvReleaseMs => {
                set_clamped_f32(&mut synth.amp_env.release_ms, value, 0.0, 10000.0, 1.0)
            }
            PluckParamId::FilterCutoffHz => {
                return self.glide_instrument_param(slot, Glide::FilterCutoff, value)
            }
            PluckParamId::FilterResonance => {
                return self.glide_instrument_param(slot, Glide::FilterResonance, value)
            }
            PluckParamId::FilterEnvAmountPct => {
                return self.glide_instrument_param(slot, Glide::FilterEnvAmount, value)
            }
            PluckParamId::FilterKeyTrackingPct => {
                return self.glide_instrument_param(slot, Glide::FilterKeyTracking, value)
            }
            PluckParamId::FilterEnvAttackMs => {
                set_clamped_f32(&mut synth.filter_env.attack_ms, value, 0.0, 5000.0, 1.0)
            }
            PluckParamId::FilterEnvDecayMs => {
                set_clamped_f32(&mut synth.filter_env.decay_ms, value, 0.0, 5000.0, 1.0)
            }
            PluckParamId::FilterEnvSustainPct => {
                set_clamped_f32(&mut synth.filter_env.sustain_pct, value, 0.0, 100.0, 1.0)
            }
            PluckParamId::FilterEnvReleaseMs => {
                set_clamped_f32(&mut synth.filter_env.release_ms, value, 0.0, 10000.0, 1.0)
            }
        };
        if mutation == ScalarMutation::Changed {
            self.refresh_synth_render_config(slot);
        }
        mutation
    }
}
