use super::*;

#[test]
fn note_off_releases_synth_after_synth_to_sample_to_none() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 36, 100, 50_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    engine.set_sample_banks(vec![sample_bank(vec![1.0; 16_384]); INSTRUMENT_SLOT_COUNT]);
    engine.note_on(0, 36, 100, 50_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            kind: "none".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );

    engine.note_off(0, 36);

    assert_eq!(engine.active_sample_voice_count_for_slot(0), 0);
    assert_eq!(engine.active_voice_count_for_slot(0), 1);
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}

#[test]
fn note_off_stops_sample_after_sample_to_synth_to_none() {
    let mut engine = multi_slot_sample_voice_engine();
    engine.note_on(0, 36, 100, 50_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    engine.note_on(0, 36, 100, 50_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            kind: "none".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );

    engine.note_off(0, 36);

    assert_eq!(engine.active_sample_voice_count_for_slot(0), 0);
    assert_eq!(engine.active_voice_count_for_slot(0), 1);
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}
