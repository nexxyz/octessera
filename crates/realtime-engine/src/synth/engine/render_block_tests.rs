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
fn parallel_synth_slot_render_matches_canonical_for_multi_slot_synth() {
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.enable_synth_slot_workers_for_tests(3);
    for (slot, note, velocity) in [(0, 60, 96), (1, 64, 88), (2, 67, 104), (3, 72, 72)] {
        block.note_on(slot, note, velocity, 1_000);
        reference.note_on(slot, note, velocity, 1_000);
    }

    assert_prepared_block_matches_reference(block, reference, 256);
}

#[test]
fn parallel_synth_slot_render_preserves_note_release_inside_block() {
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.enable_synth_slot_workers_for_tests(3);
    for (slot, note) in [(0, 60), (1, 64), (2, 67)] {
        block.note_on(slot, note, 96, 0);
        reference.note_on(slot, note, 96, 0);
    }

    assert_prepared_block_matches_reference(block, reference, 256);
}

#[test]
fn parallel_synth_slot_render_matches_canonical_across_repeated_blocks() {
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.enable_synth_slot_workers_for_tests(2);
    for (slot, note, velocity) in [(0, 60, 96), (1, 64, 88), (2, 67, 104)] {
        block.note_on(slot, note, velocity, 1_000);
        reference.note_on(slot, note, velocity, 1_000);
    }

    assert_block_matches_reference(&mut block, &mut reference, 256);
    assert_block_matches_reference(&mut block, &mut reference, 256);
    assert_block_matches_reference(&mut block, &mut reference, 256);
    let snapshot = block.profile_snapshot();
    assert!(snapshot.synth_parallel_dispatches > 0);
    assert_eq!(snapshot.synth_parallel_failures, 0);
    assert!(!snapshot.synth_parallel_unhealthy);
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
fn parallel_synth_slot_render_matches_canonical_with_samples_and_preview() {
    let mut block = sampler_preview_and_synth_engine();
    let mut reference = sampler_preview_and_synth_engine();
    block.enable_synth_slot_workers_for_tests(3);
    let preview = sample_buffer(vec![0.25, 0.5, 0.25, 0.0]);
    block.note_on(0, 36, 127, 1_000);
    reference.note_on(0, 36, 127, 1_000);
    block.preview_sample(0, preview.clone(), 100);
    reference.preview_sample(0, preview, 100);
    for (slot, note) in [(1, 60), (2, 64), (3, 67)] {
        block.note_on(slot, note, 96, 1_000);
        reference.note_on(slot, note, 96, 1_000);
    }

    assert_prepared_block_matches_reference(block, reference, 256);
}

#[test]
fn parallel_synth_slot_render_matches_canonical_for_routing_fx() {
    let config = multi_synth_delay_bus_config();
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.enable_synth_slot_workers_for_tests(3);
    block.set_instruments(config.clone());
    reference.set_instruments(config);
    for (slot, note) in [(0, 60), (1, 64), (2, 67)] {
        block.note_on(slot, note, 96, 1_000);
        reference.note_on(slot, note, 96, 1_000);
    }

    assert_prepared_block_matches_reference(block, reference, 256);
}

#[test]
fn worker_disabled_default_uses_safe_block_path_with_parity() {
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.note_on(0, 60, 96, 1_000);
    reference.note_on(0, 60, 96, 1_000);

    assert_prepared_block_matches_reference(block, reference, 128);
}

#[test]
fn parallel_synth_slot_render_skips_light_patch_without_backoff() {
    let mut block = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    block.enable_synth_slot_workers_for_tests(3);
    block.note_on(0, 60, 96, 1_000);
    reference.note_on(0, 60, 96, 1_000);

    for _ in 0..4 {
        assert_block_matches_reference(&mut block, &mut reference, 128);
    }

    assert_eq!(block.profile_snapshot().synth_parallel_dispatches, 0);
    assert_eq!(block.profile_snapshot().synth_parallel_light_skips, 4);
    assert_eq!(block.synth_parallel_backoff_blocks, 0);
    assert_eq!(block.synth_parallel_failure_count, 0);
    assert!(!block.synth_parallel_unhealthy);
}

#[test]
fn profiling_enabled_with_workers_uses_canonical_profile_path() {
    let mut block = SynthEngine::new(44_100);
    block.enable_synth_slot_workers_for_tests(3);
    block.set_render_profile_enabled(true);
    block.note_on(0, 60, 96, 1_000);
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut out = Vec::new();

    block.render_interleaved_block(32, &mut left, &mut right, &mut out);

    let snapshot = block.render_profile_snapshot();
    assert!(snapshot.enabled);
    assert_eq!(snapshot.frames_observed, 32);
    assert_eq!(snapshot.blocks_observed, 1);
}

#[test]
fn synth_slot_worker_pool_shuts_down_on_drop() {
    let mut engine = SynthEngine::new(44_100);
    engine.enable_synth_slot_workers_for_tests(3);
    drop(engine);
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
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn multi_synth_delay_bus_config() -> InstrumentsConfig {
    let mut config = delay_bus_config();
    config.instruments = (0..3)
        .map(|_| InstrumentSlotConfig {
            kind: "synth".to_string(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".to_string(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        })
        .collect();
    config
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
