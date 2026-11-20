use super::*;

#[test]
fn block_render_without_its_source_executor_stays_silent_instead_of_panicking() {
    let mut engine = SynthEngine::new(44_100);
    engine.note_on(0, 60, 100, 1_000);
    engine.block_slot_scratch.inline_source_executor = None;
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());

    engine.render_interleaved_block(128, &mut left, &mut right, &mut out);

    assert!(out.iter().all(|sample| *sample == 0.0));
}

#[test]
fn note_and_cc_events_for_a_slot_past_the_last_are_ignored_not_sent_to_slot_eight() {
    let render = |engine: &mut SynthEngine| {
        let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
        engine.render_interleaved_block(2_048, &mut left, &mut right, &mut out);
        out
    };
    let mut stray = SynthEngine::new(44_100);
    stray.note_on(8, 60, 100, 1_000);
    assert_eq!(stray.profile_snapshot().active_synth_voices, 0);
    assert!(render(&mut stray).iter().all(|sample| *sample == 0.0));

    let mut held = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    for engine in [&mut held, &mut reference] {
        engine.note_on(7, 60, 100, 1_000);
    }
    held.note_off(8, 60);
    held.cc(8, 74, 0);
    let reference_out = render(&mut reference);
    assert!(reference_out.iter().any(|sample| *sample != 0.0));
    assert_eq!(render(&mut held), reference_out);
}
