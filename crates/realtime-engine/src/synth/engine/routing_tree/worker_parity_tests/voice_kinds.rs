use super::*;
use crate::synth::{DrumParamId, FmParamId, PluckParamId, SynthParamId};

const KINDS: [&str; 5] = ["synth", "fm", "pluck", "drum", "sampler"];

fn one_shot_banks() -> Vec<SampleBankConfig> {
    let samples: Vec<f32> = (0..30_000)
        .map(|frame| (frame as f32 * 0.05).sin() * (1.0 - frame as f32 / 30_000.0) * 0.5)
        .collect();
    let mut banks = vec![SampleBankConfig::default(); INSTRUMENT_SLOT_COUNT];
    banks[0].slots[0].buffer = Some(SampleBuffer {
        samples: samples.into(),
        channels: 1,
        sample_rate: SAMPLE_BUFFER_RATE,
    });
    banks
}

fn start_voices(engine: &mut SynthEngine, kind: &str) {
    match kind {
        "drum" => {
            for voice in [0, 3, 6] {
                engine.tile_hit(0, voice, voice, 0, 110);
            }
        }
        "sampler" => {
            drop(engine.set_sample_banks(one_shot_banks()));
            for (tile, pitch_cents) in [(0, 0), (1, 700), (2, 1_200)] {
                engine.tile_hit(0, 0, tile, pitch_cents, 110);
            }
        }
        _ => {
            for (note, velocity) in [(60, 100), (67, 90), (72, 80)] {
                engine.note_on(0, note, velocity, 300);
            }
        }
    }
    let voices = engine.profile_snapshot();
    assert_eq!(
        voices.active_synth_voices + voices.active_sample_voices,
        3,
        "{kind} voices"
    );
}

fn glide_voice_param(engine: &mut SynthEngine, kind: &str) -> ScalarMutation {
    match kind {
        "synth" => engine.set_synth_param_typed(0, SynthParamId::AmpGainPct, 30.0),
        "fm" => engine.set_fm_param_typed(0, FmParamId::Index, 80.0),
        "pluck" => engine.set_pluck_param_typed(0, PluckParamId::BrightnessPct, 20.0),
        "drum" => engine.set_drum_param_typed(0, 0, DrumParamId::TonePct, 20.0),
        _ => engine.set_sample_bank_param_typed(0, SampleBankParamId::FilterCutoffHz, 700.0),
    }
}

#[test]
fn every_voice_kind_renders_the_same_on_the_routing_tree_workers() {
    for rate in RATES {
        for kind in KINDS {
            for route in ["direct", "fx_bus_1"] {
                let label = format!("{kind} via {route} at {rate} Hz");
                let mut routed = SynthEngine::new(rate);
                let mut inline = SynthEngine::new(rate);
                for engine in [&mut routed, &mut inline] {
                    engine.set_instruments(one_instrument_config(kind, route));
                    start_voices(engine, kind);
                }
                assert_worker_render_matches_inline(
                    &mut routed,
                    &mut inline,
                    &label,
                    |engine, block, routed| {
                        if block + usize::from(routed) == 20 {
                            assert_eq!(
                                glide_voice_param(engine, kind),
                                ScalarMutation::Changed,
                                "{label} glide"
                            );
                        }
                    },
                );
            }
        }
    }
}
