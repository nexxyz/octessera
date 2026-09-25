use super::super::*;
use super::set_clamped_f32;
use crate::synth::scalar_param::{DrumParamId, ScalarMutation};

impl SynthEngine {
    pub fn set_drum_param_typed(
        &mut self,
        instrument_slot: usize,
        voice: u8,
        id: DrumParamId,
        value: f32,
    ) -> ScalarMutation {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && self.routing_tree_source_event_sample_clock.is_none()
        {
            self.reject_routing_tree_mutation_for_control();
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
                _ => ScalarMutation::Rejected,
            };
        }
        let cfg = &mut self.instruments[slot];
        let mutation = match id {
            DrumParamId::AmpGainPct => {
                set_clamped_f32(&mut cfg.amp.gain_pct, value, 0.0, 100.0, 1.0)
            }
            DrumParamId::AmpVelocitySensitivityPct => set_clamped_f32(
                &mut cfg.amp.velocity_sensitivity_pct,
                value,
                0.0,
                100.0,
                1.0,
            ),
            DrumParamId::FilterCutoffHz => {
                set_clamped_f32(&mut cfg.filter.cutoff_hz, value, 20.0, 20_000.0, 1.0)
            }
            DrumParamId::FilterResonance => {
                set_clamped_f32(&mut cfg.filter.resonance, value, 0.0, 255.0, 1.0)
            }
            DrumParamId::FilterEnvAmountPct => {
                set_clamped_f32(&mut cfg.filter.env_amount_pct, value, -100.0, 100.0, 1.0)
            }
            DrumParamId::FilterKeyTrackingPct => {
                set_clamped_f32(&mut cfg.filter.key_tracking_pct, value, 0.0, 100.0, 1.0)
            }
            _ => ScalarMutation::Rejected,
        };
        if mutation == ScalarMutation::Changed {
            let mut render = SynthVoiceRenderConfig::from_config(*cfg);
            render.source = render_voice::VoiceSource::Drum;
            render.source_generation = self.synth_render_configs[slot].source_generation;
            self.synth_render_configs[slot] = render;
            self.synth_render_revisions[slot] = self.synth_render_revisions[slot].wrapping_add(1);
        }
        mutation
    }
}
