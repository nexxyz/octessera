use super::*;
use crate::synth::{DrumParamId, FmParamId, PluckParamId};

const KINDS: [&str; 4] = ["fm", "pluck", "drum", "sampler"];

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
            for voice in [0, 3] {
                engine.tile_hit(0, voice, voice, 0, 110);
            }
        }
        "sampler" => {
            drop(engine.set_sample_banks(one_shot_banks()));
            engine.note_on(0, 36, 110, 300);
            engine.note_on(0, 43, 90, 300);
        }
        _ => {
            engine.note_on(0, 60, 100, 300);
            engine.note_on(0, 67, 90, 300);
        }
    }
}

fn glide_voice_param(engine: &mut SynthEngine, kind: &str) -> ScalarMutation {
    match kind {
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
