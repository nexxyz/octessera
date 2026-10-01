use super::*;
use crate::synth::{SampleBankConfig, SampleBuffer, SampleSlotConfig};

fn sample_bank(value: f32) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![value; 256].into(),
            channels: 1,
            sample_rate: 44_100,
        }),
    };
    bank
}

#[test]
fn prepared_instrument_owner_replaces_slot_and_bank_once() {
    let mut engine = super::prepared_control_tests::single_slot_engine();
    drop(engine.apply_prepared_instrument_owner(
        0,
        prepare_instrument_slot_config(super::prepared_control_tests::test_slot("sampler", false)),
        Some(sample_bank(0.25)),
    ));
    engine.note_on(0, 36, 100, 1_000);
    assert_eq!(engine.profile_snapshot().active_sample_voices, 1);

    let prepared =
        prepare_instrument_slot_config(super::prepared_control_tests::test_slot("synth", true));
    let replacement_bank = sample_bank(0.75);
    let (retired, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.apply_prepared_instrument_owner(0, prepared, Some(replacement_bank))
        });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(engine.slot_kind[0], InstrumentKind::Synth);
    assert_eq!(engine.profile_snapshot().active_sample_voices, 0);
    assert_eq!(retired.sample_voice_count(), 1);
    assert!(retired.sample_bank.is_some());
    assert!(retired.prepared_instrument_slot.is_none());
}

#[test]
fn rejected_instrument_owner_retains_both_payloads() {
    let mut engine = super::prepared_control_tests::single_slot_engine();
    let retired = engine.apply_prepared_instrument_owner(
        INSTRUMENT_SLOT_COUNT,
        prepare_instrument_slot_config(super::prepared_control_tests::test_slot("sampler", false)),
        Some(sample_bank(0.5)),
    );
    assert!(retired.prepared_instrument_slot.is_some());
    assert!(retired.sample_bank.is_some());
    assert_eq!(engine.slot_kind[0], InstrumentKind::Synth);
}

#[test]
fn instrument_owner_without_sample_bank_clears_sample_voices_once() {
    let mut engine = super::prepared_control_tests::single_slot_engine();
    drop(engine.apply_prepared_instrument_owner(
        0,
        prepare_instrument_slot_config(super::prepared_control_tests::test_slot("sampler", false)),
        Some(sample_bank(0.25)),
    ));
    engine.note_on(0, 36, 100, 1_000);
    let retired = engine.apply_prepared_instrument_owner(
        0,
        prepare_instrument_slot_config(super::prepared_control_tests::test_slot("synth", false)),
        None,
    );
    assert_eq!(retired.sample_voice_count(), 1);
    assert_eq!(engine.profile_snapshot().active_sample_voices, 0);
}
