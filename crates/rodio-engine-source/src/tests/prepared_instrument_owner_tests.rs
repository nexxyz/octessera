use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_instrument_slot_config, InstrumentSlotConfig,
    INSTRUMENT_SLOT_COUNT, MAX_SAMPLE_VOICES_PER_SLOT, SAMPLE_VOICE_LANE_CAPACITY,
};

#[test]
fn instrument_owner_replacement_has_no_callback_memory_activity() {
    let (tx, mut source, retired_rx) = super::retirement_tests::full_sample_source();
    tx.send(EngineEvent::SetPreparedInstrumentOwner {
        instrument_slot: 0,
        generation: 1,
        config: prepare_instrument_slot_config(InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: None,
        }),
        sample_bank: Some(super::retirement_tests::sample_bank(2.0)),
    })
    .unwrap();

    super::retirement_tests::assert_no_callback_memory_activity(&mut source);
    let retired = retired_rx
        .try_recv()
        .expect("expected owner retirement")
        .state
        .expect("expected retired owner state");
    assert_eq!(
        source.engine.profile_snapshot().active_sample_voices,
        SAMPLE_VOICE_LANE_CAPACITY.min((INSTRUMENT_SLOT_COUNT - 1) * MAX_SAMPLE_VOICES_PER_SLOT)
    );
    let (allocations, deallocations) =
        super::retirement_tests::drop_retired_state_off_callback(retired);
    assert_eq!(allocations, 0);
    assert!(deallocations > 0);
}
