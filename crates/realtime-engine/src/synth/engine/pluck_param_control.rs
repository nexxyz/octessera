use super::super::*;
use super::set_clamped_f32;
use crate::synth::scalar_param::{PluckParamId, ScalarMutation};

impl SynthEngine {
    pub fn set_pluck_param_typed(
        &mut self,
        instrument_slot: usize,
        id: PluckParamId,
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
        if self.slot_kind[slot] != InstrumentKind::Pluck || !value.is_finite() {
            return ScalarMutation::Rejected;
        }
        let synth = &mut self.instruments[slot];
        let source = &mut self.synth_render_configs[slot].source;
        let render_voice::VoiceSource::Pluck {
            decay_ms,
            brightness_pct,
            pick_position_pct,
        } = source
        else {
            return ScalarMutation::Rejected;
        };
        let mutation = match id {
            PluckParamId::DecayMs => set_clamped_f32(decay_ms, value.round(), 100.0, 5_000.0, 1.0),
            PluckParamId::BrightnessPct => {
                set_clamped_f32(brightness_pct, value.round(), 0.0, 100.0, 1.0)
            }
            PluckParamId::PickPositionPct => {
                set_clamped_f32(pick_position_pct, value.round(), 5.0, 50.0, 1.0)
            }
            PluckParamId::AmpGainPct => {
                set_clamped_f32(&mut synth.amp.gain_pct, value, 0.0, 100.0, 1.0)
            }
            PluckParamId::AmpVelocitySensitivityPct => set_clamped_f32(
                &mut synth.amp.velocity_sensitivity_pct,
                value,
                0.0,
                100.0,
                1.0,
            ),
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
                set_clamped_f32(&mut synth.filter.cutoff_hz, value, 20.0, 20_000.0, 1.0)
            }
            PluckParamId::FilterResonance => {
                set_clamped_f32(&mut synth.filter.resonance, value, 0.0, 255.0, 1.0)
            }
            PluckParamId::FilterEnvAmountPct => {
                set_clamped_f32(&mut synth.filter.env_amount_pct, value, -100.0, 100.0, 1.0)
            }
            PluckParamId::FilterKeyTrackingPct => {
                set_clamped_f32(&mut synth.filter.key_tracking_pct, value, 0.0, 100.0, 1.0)
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
