use super::*;

#[test]
fn released_sample_voices_fade_out_instead_of_stopping_dead() {
    let mut engine = multi_slot_sample_voice_engine();
    engine.set_sample_banks(vec![sample_bank(vec![0.5; 48_000])]);
    engine.note_on(0, 36, 100, 60_000);
    let mut last = 0.0_f32;
    for _ in 0..2_000 {
        last = engine.next_stereo_sample().0;
    }
    assert!(
        last.abs() > 0.05,
        "the held sample should be audible: {last}"
    );
    engine.note_off(0, 36);
    let mut largest_step = 0.0_f32;
    let mut samples = Vec::new();
    for _ in 0..400 {
        let sample = engine.next_stereo_sample().0;
        largest_step = largest_step.max((sample - last).abs());
        last = sample;
        samples.push(sample);
    }
    assert!(
        largest_step < last.abs().max(0.01),
        "release jumps by {largest_step}"
    );
    assert!(samples[..200].iter().any(|sample| sample.abs() > 0.01));
    assert!(samples[260..].iter().all(|sample| sample.abs() < 1.0e-6));
    assert_eq!(engine.active_sample_voice_count_for_slot(0), 0);
}
