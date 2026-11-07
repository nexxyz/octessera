use super::prepared_control_tests::{install_prepared_config, prepared_engine};
use super::*;
use crate::synth::{FxBusConfig, SampleBankConfig, BUS_COUNT};
use std::collections::BTreeMap;

#[test]
fn rejected_routing_tree_bus_fx_retires_incoming_state() {
    let mut engine = prepared_engine(super::prepared_control_tests::test_config());
    assert!(engine.enable_routing_tree());
    engine.note_on(0, 60, 100, 1_000);

    let retired = engine.apply_prepared_fx_bus_slot(
        0,
        0,
        prepare_fx_bus_slot("reverb".into(), BTreeMap::new(), 44_100),
    );

    assert!(engine.take_routing_tree_rejection());
    assert_eq!(retired.bus_chains.len(), 1);
    assert!(matches!(
        &retired.bus_chains[0].slot_state[0],
        FxBusState::Reverb { .. }
    ));
    assert!(matches!(
        &engine.bus_chains[0].slot_state[0],
        FxBusState::Delay { .. }
    ));
}

#[test]
fn oversized_routing_tree_prepared_bus_config_is_rejected_before_mutation() {
    let mut engine = prepared_engine(super::prepared_control_tests::test_config());
    assert!(engine.enable_routing_tree());
    engine.note_on(0, 60, 100, 1_000);
    let mut oversized = super::prepared_control_tests::test_config();
    oversized.mixer.as_mut().unwrap().buses =
        (0..=BUS_COUNT).map(|_| FxBusConfig::default()).collect();

    let retired = install_prepared_config(&mut engine, oversized);

    assert!(engine.take_routing_tree_rejection());
    assert_eq!(engine.bus_chains.len(), 1);
    assert_eq!(retired.bus_chains.len(), BUS_COUNT + 1);
}

#[test]
fn instrument_owner_requires_source_clock_and_retires_both_on_rejection() {
    let mut engine = prepared_engine(super::prepared_control_tests::test_config());
    assert!(engine.enable_routing_tree());
    let rejected = engine.apply_prepared_instrument_owner(
        0,
        prepare_instrument_slot_config(super::prepared_control_tests::test_slot("sampler", false)),
        Some(SampleBankConfig::default()),
    );
    assert!(engine.take_routing_tree_rejection());
    assert!(rejected.prepared_instrument_slot.is_some());
    assert!(rejected.sample_bank.is_some());

    let accepted = engine.with_routing_tree_source_event_sample_clock(42, |engine| {
        engine.apply_prepared_instrument_owner(
            0,
            prepare_instrument_slot_config(super::prepared_control_tests::test_slot(
                "sampler", false,
            )),
            Some(SampleBankConfig::default()),
        )
    });
    assert!(!engine.take_routing_tree_rejection());
    assert!(accepted.prepared_instrument_slot.is_none());
    assert!(accepted.sample_bank.is_some());
    assert_eq!(engine.slot_kind[0], InstrumentKind::Sample);
}

#[test]
fn routing_tree_press_past_a_ringing_tail_is_admitted_and_the_tail_fades() {
    let mut engine = prepared_engine(super::prepared_control_tests::test_config());
    assert!(engine.enable_routing_tree());
    let press = |engine: &mut SynthEngine, epoch: u64, target: MomentaryFxTarget| {
        let start = prepare_momentary_fx_start_with_epoch(
            format!("fx-{epoch}"),
            epoch,
            "dub_throw".into(),
            BTreeMap::new(),
            target,
            44_100,
        )
        .unwrap();
        drop(
            engine.with_routing_tree_source_event_sample_clock(0, |engine| {
                engine.apply_prepared_momentary_fx_start(start)
            }),
        );
    };
    let ring = |engine: &mut SynthEngine, frames: usize| {
        for _ in 0..frames {
            engine.process_momentary_frame_for_test(MomentaryFxTarget::Global, 0.4, 0.4);
        }
    };
    press(&mut engine, 1, MomentaryFxTarget::Global);
    ring(&mut engine, 2_000);
    drop(
        engine.with_routing_tree_source_event_sample_clock(0, |engine| {
            engine.momentary_fx_stop_by_epoch(1)
        }),
    );
    ring(&mut engine, 64);

    press(&mut engine, 2, MomentaryFxTarget::Instrument { index: 0 });
    assert!(!engine.take_routing_tree_rejection());
    let epochs: Vec<u64> = engine.momentary_fx.iter().map(|fx| fx.epoch).collect();
    assert_eq!(epochs, vec![1, 2]);
    ring(&mut engine, 441);
    let epochs: Vec<u64> = engine.momentary_fx.iter().map(|fx| fx.epoch).collect();
    assert_eq!(epochs, vec![2]);
}
