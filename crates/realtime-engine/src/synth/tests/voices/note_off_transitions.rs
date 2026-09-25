use super::*;

#[test]
fn note_off_releases_matching_slot_note() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 100, 50_000);
    for _ in 0..64 {
        let _ = engine.next_sample();
    }
    engine.note_off(0, 60);
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}

#[test]
fn all_notes_off_releases_all_slots() {
    let mut engine = SynthEngine::new(48_000);
    for i in 0..4 {
        engine.note_on(0, 60 + i, 100, 50_000);
        engine.note_on(1, 72 + i, 100, 50_000);
    }
    engine.all_notes_off();
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
    assert_eq!(engine.active_voice_count_for_slot(1), 0);
}

#[test]
fn note_off_after_synth_to_sample_to_none_finds_no_old_synth_voice() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 36, 100, 50_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
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
            fm: None,
            pluck: None,
            drum: None,
            kind: "none".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );

    engine.note_off(0, 36);

    assert_eq!(engine.active_sample_voice_count_for_slot(0), 0);
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}

#[test]
fn note_off_after_sample_to_synth_to_none_finds_no_old_synth_voice() {
    let mut engine = multi_slot_sample_voice_engine();
    engine.note_on(0, 36, 100, 50_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    engine.note_on(0, 36, 100, 50_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "none".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );

    engine.note_off(0, 36);

    assert_eq!(engine.active_sample_voice_count_for_slot(0), 0);
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
    for _ in 0..20_000 {
        let _ = engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}
