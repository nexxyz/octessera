use super::routing_tree_executor_test_support::*;
use super::*;
use crate::synth::test_allocator;
use crate::synth::{SampleBuffer, INSTRUMENT_SLOT_COUNT};
use std::collections::BTreeMap;

#[test]
fn routing_tree_matches_canonical_across_mixed_source_blocks() {
    let config = routed_config();
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    reference.set_instruments(config);
    tree.set_sample_banks(sample_banks());
    reference.set_sample_banks(sample_banks());
    for (slot, note, velocity) in [(0, 36, 127), (1, 60, 96), (2, 64, 88), (3, 67, 80)] {
        tree.note_on(slot, note, velocity, 1_000);
        reference.note_on(slot, note, velocity, 1_000);
    }
    for frames in [64, 128, 256, 2048] {
        assert_routing_tree_matches_reference(&mut tree, &mut reference, frames);
    }
}

#[test]
fn routing_tree_matches_canonical_for_sample_source() {
    let config = sample_direct_config();
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    reference.set_instruments(config);
    tree.set_sample_banks(sample_banks());
    reference.set_sample_banks(sample_banks());
    let preview = SampleBuffer {
        samples: vec![0.25, 0.5, 0.25, 0.0].into(),
        channels: 1,
        sample_rate: 48_000,
    };
    tree.preview_sample(0, preview.clone(), 100);
    reference.preview_sample(0, preview, 100);
    tree.note_on(0, 36, 127, 1_000);
    reference.note_on(0, 36, 127, 1_000);
    assert_routing_tree_matches_reference(&mut tree, &mut reference, 64);
}

#[test]
fn routing_tree_reuses_canonical_source_kernels() {
    let config = routed_config();
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    reference.set_instruments(config);
    for (slot, note, velocity) in [(0, 60, 100), (1, 64, 96)] {
        tree.note_on(slot, note, velocity, 1_000);
        reference.note_on(slot, note, velocity, 1_000);
    }
    assert!(tree.block_slot_scratch.prepare_output(64));
    assert!(tree.render_block_sources_for_test(64));
    for frame in 0..64 {
        let mut expected = [0.0; INSTRUMENT_SLOT_COUNT];
        reference.render_sample_voices(&mut expected);
        reference.render_preview_sample_voices(&mut expected);
        reference.render_synth_voices(&mut expected);
        for (slot, expected_sample) in expected.iter().enumerate() {
            let actual = tree.block_slot_scratch.sample_slot_out[slot][frame]
                + tree.block_slot_scratch.synth_slot_out[slot][frame];
            assert_eq!(actual, *expected_sample, "slot {slot}, frame {frame}");
        }
    }
}

#[test]
fn routing_tree_matches_canonical_for_sample_bus_source() {
    let config = sample_bus_config();
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    reference.set_instruments(config);
    tree.set_sample_banks(sample_banks());
    reference.set_sample_banks(sample_banks());
    tree.note_on(0, 36, 127, 1_000);
    reference.note_on(0, 36, 127, 1_000);
    assert_routing_tree_matches_reference(&mut tree, &mut reference, 64);
}

#[test]
fn routing_tree_matches_canonical_for_single_synth_bus_source() {
    let mut config = routed_config();
    for slot in 1..config.instruments.len() {
        config.instruments[slot].kind = "none".into();
    }
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    reference.set_instruments(config);
    tree.note_on(0, 60, 100, 1_000);
    reference.note_on(0, 60, 100, 1_000);
    assert_routing_tree_matches_reference(&mut tree, &mut reference, 256);
}

#[test]
fn routing_tree_matches_canonical_for_spread_and_auto_pan_bus_output() {
    let config = stereo_bus_config();
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    reference.set_instruments(config);
    tree.note_on(0, 60, 100, 1_000);
    reference.note_on(0, 60, 100, 1_000);
    assert_routing_tree_matches_reference(&mut tree, &mut reference, 256);
}

#[test]
fn routing_tree_matches_canonical_for_instrument_and_bus_momentary_targets() {
    let config = routed_config();
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    reference.set_instruments(config);
    tree.set_sample_banks(sample_banks());
    reference.set_sample_banks(sample_banks());
    for (slot, note) in [(0, 36), (1, 60)] {
        tree.note_on(slot, note, 100, 1_000);
        reference.note_on(slot, note, 100, 1_000);
    }
    for (id, target) in [
        ("instrument", MomentaryFxTarget::Instrument { index: 1 }),
        ("bus", MomentaryFxTarget::FxBus { index: 0 }),
    ] {
        tree.momentary_fx_start(id.into(), "filter_sweep".into(), BTreeMap::new(), target);
        reference.momentary_fx_start(id.into(), "filter_sweep".into(), BTreeMap::new(), target);
    }
    for frames in [64, 128] {
        assert_routing_tree_matches_reference(&mut tree, &mut reference, frames);
    }
}

#[test]
fn routing_tree_matches_canonical_with_master_fx_and_global_momentary_in_order() {
    let config = master_fx_config();
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    reference.set_instruments(config);
    tree.set_sample_banks(sample_banks());
    reference.set_sample_banks(sample_banks());
    tree.note_on(0, 36, 127, 1_000);
    reference.note_on(0, 36, 127, 1_000);
    for engine in [&mut tree, &mut reference] {
        engine.momentary_fx_start(
            "global".into(),
            "filter_sweep".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
        );
    }
    for frames in [64, 128] {
        assert_routing_tree_matches_reference(&mut tree, &mut reference, frames);
    }
    assert_eq!(
        format!("{:?}", tree.master_slot_state),
        format!("{:?}", reference.master_slot_state)
    );
}

#[test]
fn routing_tree_matches_canonical_for_mixed_direct_sources() {
    let mut config = routed_config();
    config.mixer = None;
    for instrument in &mut config.instruments {
        instrument.mixer.as_mut().unwrap().route = "direct".into();
    }
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    reference.set_instruments(config);
    tree.set_sample_banks(sample_banks());
    reference.set_sample_banks(sample_banks());
    for (slot, note, velocity) in [(0, 36, 127), (1, 60, 96), (2, 64, 88), (3, 67, 80)] {
        tree.note_on(slot, note, velocity, 1_000);
        reference.note_on(slot, note, velocity, 1_000);
    }
    assert_routing_tree_matches_reference(&mut tree, &mut reference, 64);
}

#[test]
fn routing_tree_duck_sources_use_raw_instrument_and_pre_chain_bus_inputs() {
    let config = raw_duck_config();
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    let mut raw = SynthEngine::new(48_000);
    let mut banks = sample_banks();
    banks[1] = banks[0].clone();
    banks[2] = banks[0].clone();
    tree.set_instruments(config.clone());
    reference.set_instruments(config.clone());
    raw.set_instruments(config);
    tree.set_sample_banks(banks.clone());
    reference.set_sample_banks(banks.clone());
    raw.set_sample_banks(banks);
    for engine in [&mut tree, &mut reference, &mut raw] {
        engine.note_on(0, 36, 100, 1_000);
        engine.note_on(1, 36, 100, 1_000);
        engine.note_on(2, 36, 100, 1_000);
    }
    tree.momentary_fx_start(
        "source".into(),
        "freeze".into(),
        BTreeMap::new(),
        MomentaryFxTarget::Instrument { index: 1 },
    );
    reference.momentary_fx_start(
        "source".into(),
        "freeze".into(),
        BTreeMap::new(),
        MomentaryFxTarget::Instrument { index: 1 },
    );
    let mut raw_slots = [0.0; INSTRUMENT_SLOT_COUNT];
    raw.render_sample_voices(&mut raw_slots);
    raw.render_synth_voices(&mut raw_slots);
    let raw_instrument_source = raw_slots[1];
    let raw_bus_source = raw_slots[2] * 0.4;
    assert!(raw_instrument_source.abs() > 0.0);
    assert!(raw_bus_source.abs() > 0.0);
    assert_ne!(raw_instrument_source, raw_instrument_source * 0.25);
    assert_ne!(raw_bus_source, raw_bus_source * 0.7);
    let mut actual_left = [0.0];
    let mut actual_right = [0.0];
    assert!(tree.render_routing_tree_block_for_test(1, &mut actual_left, &mut actual_right));
    let (expected_left, expected_right) = reference.next_stereo_sample();
    assert_eq!(actual_left[0], expected_left);
    assert_eq!(actual_right[0], expected_right);
    assert_momentary_state_matches(&tree, &reference);
    assert_eq!(
        tree.block_slot_scratch.sample_slot_out[1][0],
        raw_instrument_source
    );
    let (instrument_attack, bus_attack) = match (
        tree.bus_chains[0].slot_params[0],
        tree.bus_chains[0].slot_params[1],
    ) {
        (
            FxBusParams::Duck {
                attack_ms: instrument_attack,
                ..
            },
            FxBusParams::Duck {
                attack_ms: bus_attack,
                ..
            },
        ) => (instrument_attack, bus_attack),
        _ => panic!("expected duck slots"),
    };
    assert_duck_env(
        &tree.bus_chains[0].slot_state[0],
        raw_instrument_source,
        instrument_attack,
        "instrument",
    );
    assert_duck_env(
        &tree.bus_chains[0].slot_state[1],
        raw_bus_source,
        bus_attack,
        "bus",
    );
}

#[test]
fn routing_tree_preserves_state_after_note_off_and_tail_blocks() {
    let config = routed_config();
    let mut tree = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    reference.set_instruments(config);
    for (slot, note) in [(0, 60), (1, 64)] {
        tree.note_on(slot, note, 100, 1_000);
        reference.note_on(slot, note, 100, 1_000);
    }
    assert_routing_tree_matches_reference(&mut tree, &mut reference, 256);
    tree.note_off(0, 60);
    reference.note_off(0, 60);
    tree.note_off(1, 64);
    reference.note_off(1, 64);
    for _ in 0..4 {
        assert_routing_tree_matches_reference(&mut tree, &mut reference, 256);
    }
    assert_eq!(tree.sample_clock, reference.sample_clock);
    assert_eq!(
        tree.active_bus_activity_count,
        reference.active_bus_activity_count
    );
    assert_eq!(
        tree.master_activity_frames,
        reference.master_activity_frames
    );
}

#[test]
fn routing_tree_block_render_does_not_allocate_after_warmup() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(routed_config());
    engine.set_sample_banks(sample_banks());
    engine.note_on(0, 36, 127, 1_000);
    engine.note_on(1, 60, 100, 1_000);
    for frames in [32, 64, 128, 256] {
        let mut left = vec![0.0; frames];
        let mut right = vec![0.0; frames];
        assert!(engine.render_routing_tree_block_for_test(frames, &mut left, &mut right));
        let (_, allocations, deallocations) =
            test_allocator::count_allocations_and_deallocations(|| {
                assert!(engine.render_routing_tree_block_for_test(frames, &mut left, &mut right));
            });
        assert_eq!((allocations, deallocations), (0, 0), "frames {frames}");
    }
}
