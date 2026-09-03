use super::super::source_worker_bus::{apply_bus_block, render_bus_block, stage_bus_block};
use super::super::*;
use super::{assert_bits_equal, configured_delay_with_spread, configured_fx, inline_bus_output};
use super::{one_bus, slot_out, staged_bus_output};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn every_fx_kind_has_a_bit_exact_block_kernel() {
    for kind in [
        "none",
        "tremolo",
        "delay",
        "vibrato",
        "chorus",
        "flanger",
        "filter_lfo",
        "wah",
        "reverb",
        "glitch",
        "auto_pan",
        "duck",
        "saturator",
        "distortion",
        "bitcrusher",
        "compressor",
        "eq",
        "vinyl",
    ] {
        let config = one_bus(vec![
            configured_fx(kind),
            configured_fx("none"),
            configured_fx("none"),
        ]);
        let slot_out = slot_out(128, 0.4);
        let mut reference = SynthEngine::new(48_000);
        reference.set_instruments(config.clone());
        let expected = inline_bus_output(&mut reference, &slot_out, 128);
        let actual = staged_bus_output(config, &slot_out, 128, false, None);
        assert_bits_equal(&actual, &expected);
    }
}

#[test]
fn four_buses_and_twelve_effects_match_logical_inline_order() {
    let config = super::bus_config(
        (0..4)
            .map(|_| {
                vec![
                    configured_fx("delay"),
                    configured_fx("duck"),
                    configured_fx("reverb"),
                ]
            })
            .collect(),
        "fx_bus_1",
    );
    let slot_out = slot_out(256, 0.35);
    let mut reference = SynthEngine::new(48_000);
    reference.set_instruments(config.clone());
    let expected = inline_bus_output(&mut reference, &slot_out, 256);
    let actual = staged_bus_output(config, &slot_out, 256, false, None);
    assert_bits_equal(&actual, &expected);
}

#[test]
fn duck_staging_resolves_instrument_self_cross_and_is_order_independent() {
    let config = super::bus_config(
        vec![
            vec![
                FxBusSlotConfig::Config {
                    kind: "duck".into(),
                    params: BTreeMap::from([(String::from("source"), json!("I1"))]),
                },
                configured_fx("none"),
                configured_fx("none"),
            ],
            vec![
                FxBusSlotConfig::Config {
                    kind: "duck".into(),
                    params: BTreeMap::from([(String::from("source"), json!("B1"))]),
                },
                configured_fx("none"),
                configured_fx("none"),
            ],
        ],
        "fx_bus_1",
    );
    let slot_out = slot_out(128, 0.7);
    let mut reference = SynthEngine::new(48_000);
    reference.set_instruments(config.clone());
    let expected = inline_bus_output(&mut reference, &slot_out, 128);
    let actual = staged_bus_output(config.clone(), &slot_out, 128, false, None);
    let reversed = staged_bus_output(config, &slot_out, 128, true, None);
    assert_bits_equal(&actual, &expected);
    assert_bits_equal(&reversed, &expected);
}

#[test]
fn delay_reverb_glitch_and_vinyl_tails_match_across_blocks() {
    for kind in ["delay", "reverb", "glitch", "vinyl"] {
        let config = one_bus(vec![
            configured_fx(kind),
            configured_fx("none"),
            configured_fx("none"),
        ]);
        let mut reference = SynthEngine::new(48_000);
        reference.set_instruments(config.clone());
        let first = slot_out(128, 0.8);
        let second = slot_out(128, 0.0);
        let expected_first = inline_bus_output(&mut reference, &first, 128);
        let expected_second = inline_bus_output(&mut reference, &second, 128);

        let mut engine = SynthEngine::new(48_000);
        engine.set_instruments(config);
        let (lifecycle, runtime) =
            SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
        let mut owners = runtime.take_home_owners_for_test().expect("owner pair");
        let mut actual_blocks = Vec::new();
        for block in [&first, &second] {
            assert!(stage_bus_block(&mut engine, &mut owners, block, 128));
            let stamp = runtime.stamp_for_test(&engine, 128);
            for owner in &mut owners {
                assert!(render_bus_block(
                    owner,
                    owner.parity,
                    stamp,
                    128,
                    engine.sample_rate,
                    engine.dsp_config.bus_idle_threshold,
                    engine.fx_activity_hold_frames,
                )
                .is_ok());
            }
            let mut left = vec![0.0; 128];
            let mut right = vec![0.0; 128];
            assert!(apply_bus_block(
                &mut engine,
                &owners,
                128,
                &mut left,
                &mut right
            ));
            actual_blocks.extend(left.into_iter().zip(right).flat_map(|(l, r)| [l, r]));
        }
        runtime.return_home_owners_for_test(owners);
        assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
        let mut expected = expected_first;
        expected.extend(expected_second);
        assert_bits_equal(&actual_blocks, &expected);
    }
}

#[test]
fn auto_pan_and_spread_are_written_to_carrier_metadata() {
    let config = one_bus(vec![
        configured_delay_with_spread(),
        configured_fx("auto_pan"),
        configured_fx("none"),
    ]);
    let slot_out = slot_out(128, 0.4);
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(config);
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let mut owners = runtime.take_home_owners_for_test().expect("owner pair");
    assert!(stage_bus_block(&mut engine, &mut owners, &slot_out, 128));
    let stamp = runtime.stamp_for_test(&engine, 128);
    assert!(render_bus_block(
        &mut owners[0],
        0,
        stamp,
        128,
        engine.sample_rate,
        engine.dsp_config.bus_idle_threshold,
        engine.fx_activity_hold_frames,
    )
    .is_ok());
    let carrier = owners[0]
        .bus_carriers
        .iter()
        .flatten()
        .find(|carrier| carrier.logical_bus_id == 0)
        .expect("bus zero carrier");
    assert!(carrier.scratch.executed);
    assert_eq!(carrier.scratch.processed_prefix, 128);
    assert!(carrier.scratch.spread > 0.0);
    assert!(carrier.scratch.auto_pan_pos[..128]
        .iter()
        .any(|position| !position.is_nan()));
    runtime.return_home_owners_for_test(owners);
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn bus_block_kernel_has_no_callback_memory_activity() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(one_bus(vec![
        configured_fx("reverb"),
        configured_fx("reverb"),
        configured_fx("reverb"),
    ]));
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let mut owners = runtime.take_home_owners_for_test().expect("owner pair");
    let slot_out = slot_out(128, 0.5);
    assert!(stage_bus_block(&mut engine, &mut owners, &slot_out, 128));
    let stamp = runtime.stamp_for_test(&engine, 128);
    let sample_rate = engine.sample_rate;
    let threshold = engine.dsp_config.bus_idle_threshold;
    let hold_frames = engine.fx_activity_hold_frames;
    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            for owner in &mut owners {
                assert!(render_bus_block(
                    owner,
                    owner.parity,
                    stamp,
                    128,
                    sample_rate,
                    threshold,
                    hold_frames,
                )
                .is_ok());
            }
        });
    assert_eq!((allocations, deallocations), (0, 0));
    runtime.return_home_owners_for_test(owners);
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn staged_momentary_and_spread_match_inline_at_all_supported_block_sizes() {
    for frames in [64, 128, 256, 2048] {
        let config = one_bus(vec![
            configured_delay_with_spread(),
            configured_fx("none"),
            configured_fx("none"),
        ]);
        let slot_out = slot_out(frames, 0.35);
        let mut reference = SynthEngine::new(48_000);
        reference.set_instruments(config.clone());
        let params = BTreeMap::from([
            ("cutoffPct".into(), json!(60.0)),
            ("sweepInMs".into(), json!(1.0)),
        ]);
        super::install_momentary(
            &mut reference,
            "instrument",
            "filter_sweep",
            MomentaryFxTarget::Instrument { index: 0 },
            params.clone(),
        );
        let expected = inline_bus_output(&mut reference, &slot_out, frames);
        let actual = staged_bus_output(
            config,
            &slot_out,
            frames,
            false,
            Some((
                "instrument",
                "filter_sweep",
                MomentaryFxTarget::Instrument { index: 0 },
                params,
            )),
        );
        assert_bits_equal(&actual, &expected);
    }
}
