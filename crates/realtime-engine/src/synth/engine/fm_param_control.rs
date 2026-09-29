use super::super::*;
use super::set_clamped_f32;
use crate::synth::scalar_param::{FmParamId, ScalarMutation};

impl SynthEngine {
    pub fn set_fm_param_typed(
        &mut self,
        instrument_slot: usize,
        id: FmParamId,
        value: f32,
    ) -> ScalarMutation {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_none()
        {
            self.reject_routing_tree_mutation_for_control();
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
            index,
            velocity_to_index,
            mod_shape,
            mod_mix,
            index_env,
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
                set_clamped_f32(index, value.round().clamp(0.0, 100.0) * 0.04, 0.0, 4.0, 1.0)
            }
            FmParamId::VelocityToIndexPct => {
                set_clamped_f32(velocity_to_index, value.round() * 0.01, 0.0, 1.0, 1.0)
            }
            FmParamId::ModShapePct => {
                set_clamped_f32(mod_shape, value.round() * 0.01, 0.0, 1.0, 1.0)
            }
            FmParamId::ModMixPct => set_clamped_f32(mod_mix, value.round() * 0.01, 0.0, 1.0, 1.0),
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
                set_clamped_f32(&mut synth.amp.gain_pct, value, 0.0, 100.0, 1.0)
            }
            FmParamId::AmpVelocitySensitivityPct => set_clamped_f32(
                &mut synth.amp.velocity_sensitivity_pct,
                value,
                0.0,
                100.0,
                1.0,
            ),
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
                set_clamped_f32(&mut synth.filter.cutoff_hz, value, 20.0, 20_000.0, 1.0)
            }
            FmParamId::FilterResonance => {
                set_clamped_f32(&mut synth.filter.resonance, value, 0.0, 255.0, 1.0)
            }
            FmParamId::FilterEnvAmountPct => {
                set_clamped_f32(&mut synth.filter.env_amount_pct, value, -100.0, 100.0, 1.0)
            }
            FmParamId::FilterKeyTrackingPct => {
                set_clamped_f32(&mut synth.filter.key_tracking_pct, value, 0.0, 100.0, 1.0)
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
            let source = *source;
            let mut render = SynthVoiceRenderConfig::from_config(*synth);
            render.source = source;
            render.source_generation = self.synth_render_configs[slot].source_generation;
            self.synth_render_configs[slot] = render;
            self.synth_render_revisions[slot] = self.synth_render_revisions[slot].wrapping_add(1);
        }
        mutation
    }
}
