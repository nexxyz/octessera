use super::*;
use crate::synth::{
    FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MixerConfig, SampleBankConfig, SampleBuffer, SampleSlotConfig,
};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn prepared_block_slot_render_matches_canonical_for_multi_slot_synth() {
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    for (slot, note, velocity) in [(0, 60, 96), (1, 64, 88), (2, 67, 104), (3, 72, 72)] {
        block.note_on(slot, note, velocity, 1_000);
        reference.note_on(slot, note, velocity, 1_000);
    }

    assert_prepared_block_matches_reference(block, reference, 256);
}

#[test]
fn prepared_block_slot_render_matches_canonical_for_multi_slot_samples() {
    let mut block = multi_slot_sample_engine();
    let mut reference = multi_slot_sample_engine();
    for (slot, velocity) in [(0, 127), (1, 96), (2, 80), (3, 112)] {
        block.note_on(slot, 36, velocity, 1_000);
        reference.note_on(slot, 36, velocity, 1_000);
    }
    assert_eq!(block.profile_snapshot().active_sample_voices, 4);
    assert_eq!(reference.profile_snapshot().active_sample_voices, 4);

    assert_prepared_block_matches_reference(block, reference, 8);
}

#[test]
fn prepared_block_slot_render_preserves_sample_end_inside_block() {
    let mut block = multi_slot_sample_engine();
    let mut reference = multi_slot_sample_engine();
    block.note_on(0, 36, 127, 1_000);
    reference.note_on(0, 36, 127, 1_000);

    assert_prepared_block_matches_reference(block, reference, 16);
}

#[test]
fn sample_block_render_matches_serial_after_lane_reuse() {
    let mut block = multi_slot_sample_engine();
    let mut reference = multi_slot_sample_engine();
    block.note_on(0, 36, 127, 1_000);
    reference.note_on(0, 36, 127, 1_000);
    assert_block_matches_reference(&mut block, &mut reference, 8);

    block.note_on(1, 36, 127, 1_000);
    reference.note_on(1, 36, 127, 1_000);
    assert_block_matches_reference(&mut block, &mut reference, 8);
}

#[test]
fn prepared_block_slot_render_matches_canonical_for_routing_fx() {
    let config = delay_bus_config();
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.set_instruments(config.clone());
    reference.set_instruments(config);
    block.note_on(0, 60, 96, 1_000);
    reference.note_on(0, 60, 96, 1_000);

    assert_prepared_block_matches_reference(block, reference, 256);
}

#[test]
fn prepared_block_slot_render_matches_canonical_with_preview_active() {
    let mut block = sampler_preview_and_synth_engine();
    let mut reference = sampler_preview_and_synth_engine();
    let preview = sample_buffer(vec![0.25, 0.5, 0.25, 0.0]);
    block.note_on(0, 36, 127, 1_000);
    reference.note_on(0, 36, 127, 1_000);
    block.preview_sample(0, preview.clone(), 100);
    reference.preview_sample(0, preview, 100);
    block.note_on(1, 60, 96, 1_000);
    reference.note_on(1, 60, 96, 1_000);
    assert_eq!(block.profile_snapshot().active_sample_voices, 1);
    assert_eq!(block.profile_snapshot().active_preview_sample_voices, 1);

    assert_prepared_block_matches_reference(block, reference, 8);
}

#[test]
fn default_block_render_uses_inline_source_path_with_parity() {
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.note_on(0, 60, 96, 1_000);
    reference.note_on(0, 60, 96, 1_000);

    assert_prepared_block_matches_reference(block, reference, 128);
}

#[test]
fn mixed_full_voice_pool_inline_render_matches_serial_at_supported_quanta() {
    for frames in [64, 128, 256, 2048] {
        let mut block = mixed_full_voice_engine();
        let mut reference = mixed_full_voice_engine();
        for slot in 0..8 {
            for note in 0..8 {
                block.note_on(slot, 48 + note, 96, 5_000);
                reference.note_on(slot, 48 + note, 96, 5_000);
            }
        }
        for slot in 0..8 {
            let sampler = InstrumentSlotConfig {
                kind: "sampler".to_string(),
                synth: default_synth_config(),
                mixer: None,
            };
            block.set_instrument_slot(slot, sampler.clone());
            reference.set_instrument_slot(slot, sampler);
            for _ in 0..8 {
                block.note_on(slot as u8, 36, 96, 5_000);
                reference.note_on(slot as u8, 36, 96, 5_000);
            }
        }
        assert_eq!(block.profile_snapshot().active_synth_voices, 64);
        assert_eq!(block.profile_snapshot().active_sample_voices, 64);
        assert_prepared_block_matches_reference(block, reference, frames);
    }
}

#[test]
fn inline_source_kernels_match_serial_with_dynamic_voice_state_at_supported_quanta() {
    for frames in [64, 128, 256, 2048] {
        let mut block = dynamic_source_engine();
        let mut reference = dynamic_source_engine();
        block.note_on(0, 36, 111, 5_000);
        reference.note_on(0, 36, 111, 5_000);
        block.note_on(1, 60, 97, 5_000);
        reference.note_on(1, 60, 97, 5_000);

        for engine in [&mut block, &mut reference] {
            engine.set_sample_bank_param(0, "sample.filter.cutoffHz", 1_700.0);
            engine.set_sample_bank_param(0, "sample.filter.resonance", 61.0);
            engine.set_synth_param(1, "synth.filter.cutoffHz", 2_300.0);
            engine.cc(1, 74, 91);
            engine.cc(1, 71, 83);
        }
        assert_eq!(
            block.synth_render_revisions,
            reference.synth_render_revisions
        );
        assert_eq!(block.synth_render_revisions[1], 2);
        assert_block_matches_reference(&mut block, &mut reference, frames);

        block.note_off(0, 36);
        reference.note_off(0, 36);
        block.note_off(1, 60);
        reference.note_off(1, 60);
        assert_block_matches_reference(&mut block, &mut reference, frames);
    }
}

#[test]
fn inline_quantum_matches_serial_when_synth_voice_ends_mid_quantum() {
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.note_on(0, 60, 96, 0);
    reference.note_on(0, 60, 96, 0);

    assert_prepared_block_matches_reference(block, reference, 128);
}

#[test]
fn inline_quantum_matches_serial_after_note_off_within_sequence() {
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.note_on(0, 60, 96, 1_000);
    reference.note_on(0, 60, 96, 1_000);
    let mut warm_left = Vec::new();
    let mut warm_right = Vec::new();
    let mut warm_out = Vec::new();
    block.render_interleaved_block(32, &mut warm_left, &mut warm_right, &mut warm_out);
    for _ in 0..32 {
        let _ = reference.next_stereo_sample();
    }
    block.note_off(0, 60);
    reference.note_off(0, 60);

    assert_block_matches_reference(&mut block, &mut reference, 128);
}

#[test]
fn inline_reduction_is_independent_of_physical_allocation_history() {
    let mut direct_synth = SynthEngine::new(44_100);
    let mut displaced_synth = SynthEngine::new(44_100);
    displaced_synth.slot_volume[1] = 0.0;
    direct_synth.note_on(0, 60, 96, 1_000);
    direct_synth.note_on(0, 64, 96, 1_000);
    displaced_synth.note_on(1, 72, 96, 1_000);
    displaced_synth.note_on(0, 60, 96, 1_000);
    displaced_synth.note_on(0, 64, 96, 1_000);
    assert_inline_blocks_match(&mut direct_synth, &mut displaced_synth, 128);

    let mut direct_sample = multi_slot_sample_engine();
    let mut displaced_sample = multi_slot_sample_engine();
    displaced_sample.slot_volume[1] = 0.0;
    direct_sample.note_on(0, 36, 127, 1_000);
    direct_sample.note_on(0, 36, 127, 1_000);
    displaced_sample.note_on(1, 36, 127, 1_000);
    displaced_sample.note_on(0, 36, 127, 1_000);
    displaced_sample.note_on(0, 36, 127, 1_000);
    assert_inline_blocks_match(&mut direct_sample, &mut displaced_sample, 8);
}

#[test]
fn inline_quantum_preserves_active_voices_across_omitted_type_and_route_edits() {
    let mut block = sampler_preview_and_synth_engine();
    let mut reference = sampler_preview_and_synth_engine();
    block.note_on(0, 36, 127, 1_000);
    reference.note_on(0, 36, 127, 1_000);
    block.note_on(1, 60, 96, 1_000);
    reference.note_on(1, 60, 96, 1_000);

    let omitted = InstrumentsConfig {
        instruments: Vec::new(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    };
    block.set_instruments(omitted.clone());
    reference.set_instruments(omitted);
    let next = InstrumentSlotConfig {
        kind: "synth".into(),
        synth: default_synth_config(),
        mixer: Some(InstrumentMixerConfig {
            route: "direct".into(),
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume: 100.0,
        }),
    };
    block.set_instrument_slot(0, next.clone());
    reference.set_instrument_slot(0, next);
    assert_block_matches_reference(&mut block, &mut reference, 128);
}

#[test]
fn inline_source_executor_does_not_allocate_at_default_quantum() {
    let mut engine = SynthEngine::new(44_100);
    engine.note_on(0, 60, 96, 1_000);
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    engine.render_interleaved_block(128, &mut left, &mut right, &mut out);

    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
        });
    assert_eq!(allocations, 0);
    assert_eq!(deallocations, 0);
}

#[test]
fn oversized_block_slot_render_falls_back_to_canonical() {
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.note_on(0, 60, 96, 1_000);
    reference.note_on(0, 60, 96, 1_000);

    assert_block_matches_reference(&mut block, &mut reference, BLOCK_SLOT_SCRATCH_FRAMES + 1);
}

fn assert_prepared_block_matches_reference(
    mut block: SynthEngine,
    mut reference: SynthEngine,
    frames: usize,
) {
    assert!(block.block_slot_scratch.prepare(frames));
    assert_block_matches_reference(&mut block, &mut reference, frames);
}

fn assert_block_matches_reference(
    block: &mut SynthEngine,
    reference: &mut SynthEngine,
    frames: usize,
) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut out = Vec::new();
    block.render_interleaved_block(frames, &mut left, &mut right, &mut out);
    let mut expected = Vec::with_capacity(frames * 2);
    for _ in 0..frames {
        let (l, r) = reference.next_stereo_sample();
        expected.push(l);
        expected.push(r);
    }
    assert_eq!(out.len(), expected.len());
    for (idx, (actual, expected)) in out.iter().zip(expected).enumerate() {
        assert_eq!(actual.to_bits(), expected.to_bits(), "sample {idx}");
    }
}

fn assert_inline_blocks_match(first: &mut SynthEngine, second: &mut SynthEngine, frames: usize) {
    let mut first_left = Vec::new();
    let mut first_right = Vec::new();
    let mut first_out = Vec::new();
    let mut second_left = Vec::new();
    let mut second_right = Vec::new();
    let mut second_out = Vec::new();
    first.render_interleaved_block(frames, &mut first_left, &mut first_right, &mut first_out);
    second.render_interleaved_block(frames, &mut second_left, &mut second_right, &mut second_out);
    assert_eq!(first_out.len(), second_out.len());
    for (actual, expected) in first_out.iter().zip(second_out) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
}

fn delay_bus_config() -> InstrumentsConfig {
    let synth = default_synth_config();
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".to_string(),
            synth,
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "delay".to_string(),
                    params: [
                        ("timeMs".to_string(), json!(35.0)),
                        ("feedback".to_string(), json!(0.25)),
                        ("mixPct".to_string(), json!(35.0)),
                    ]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                }],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn multi_slot_sample_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: (0..INSTRUMENT_SLOT_COUNT)
            .map(|_| InstrumentSlotConfig {
                kind: "sampler".to_string(),
                synth: default_synth_config(),
                mixer: None,
            })
            .collect(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.set_sample_banks(
        (0..INSTRUMENT_SLOT_COUNT)
            .map(|slot| sample_bank(vec![1.0 - slot as f32 * 0.1, 0.5, 0.25, 0.0]))
            .collect(),
    );
    engine
}

fn sampler_preview_and_synth_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "sampler".to_string(),
                synth: default_synth_config(),
                mixer: None,
            },
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: None,
            },
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: None,
            },
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: None,
            },
        ],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.set_sample_banks(vec![sample_bank(vec![1.0, 0.5, 0.25, 0.0])]);
    engine
}

fn mixed_full_voice_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: (0..8)
            .map(|_| InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth: default_synth_config(),
                mixer: None,
            })
            .collect(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let _ = engine.set_sample_banks((0..8).map(|_| sample_bank(vec![0.25; 16_384])).collect());
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    engine
}

fn dynamic_source_engine() -> SynthEngine {
    let mut synth = default_synth_config();
    synth.filter.cutoff_hz = 1_100.0;
    synth.filter.resonance = 48.0;
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "sampler".to_string(),
                synth,
                mixer: None,
            },
            InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth,
                mixer: None,
            },
        ],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut bank = sample_bank(
        (0..4_096)
            .map(|index| (index as f32 * 0.013).sin())
            .collect(),
    );
    bank.tune_semis = 7.0;
    bank.gain_pct = 83.0;
    bank.velocity_sensitivity_pct = 37.0;
    bank.filter_cutoff_hz = 1_200.0;
    bank.filter_resonance = 54.0;
    let _ = engine.set_sample_banks(vec![bank, SampleBankConfig::default()]);
    engine
}

fn sample_bank(samples: Vec<f32>) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(sample_buffer(samples)),
    };
    bank
}

fn sample_buffer(samples: Vec<f32>) -> SampleBuffer {
    SampleBuffer {
        samples: samples.into(),
        channels: 1,
        sample_rate: 48_000,
    }
}
