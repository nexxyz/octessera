use super::*;

fn slot_config(kind: &str) -> InstrumentSlotConfig {
    InstrumentSlotConfig {
        fm: None,
        pluck: None,
        drum: None,
        kind: kind.into(),
        synth: default_synth_config(),
        mixer: None,
    }
}

fn latch_filter_ccs(engine: &mut SynthEngine) {
    engine.cc(0, 74, 127);
    engine.cc(0, 71, 64);
    assert!(engine.mod_values_for_slot(0).0.is_some());
}

#[test]
fn the_patch_load_panic_clears_every_latched_filter_cc() {
    let mut engine = SynthEngine::new(48_000);
    latch_filter_ccs(&mut engine);
    engine.cc(1, 74, 10);

    engine.all_notes_off();

    assert_eq!(engine.mod_values_for_slot(0), (None, None));
    assert_eq!(engine.mod_values_for_slot(1), (None, None));
}

#[test]
fn changing_the_instrument_kind_clears_the_latched_filter_ccs() {
    let mut engine = SynthEngine::new(48_000);
    latch_filter_ccs(&mut engine);

    engine.set_instrument_slot(0, slot_config("fm"));

    assert_eq!(engine.mod_values_for_slot(0), (None, None));
}

#[test]
fn reapplying_the_same_instrument_kind_keeps_the_latched_filter_ccs() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instrument_slot(0, slot_config("synth"));
    latch_filter_ccs(&mut engine);
    let latched = engine.mod_values_for_slot(0);

    engine.set_instrument_slot(0, slot_config("synth"));

    assert_eq!(engine.mod_values_for_slot(0), latched);
}
