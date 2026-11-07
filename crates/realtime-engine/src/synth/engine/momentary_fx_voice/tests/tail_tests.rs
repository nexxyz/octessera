use super::*;

#[test]
fn a_new_press_takes_over_a_ringing_dub_throw_without_a_cut() {
    let throw = params(&[
        ("timePulses", 12.0),
        ("feedbackPct", 70.0),
        ("mixPct", 100.0),
    ]);
    let mut states = state("dub_throw", &throw);
    let mut output = Vec::new();
    let mut retired: Retired = std::array::from_fn(|_| None);
    for frame in 0..30_000 {
        if frame == 4_000 {
            states[0].stop();
        }
        if frame == 20_000 {
            let mut incoming = MomentaryFxState::new_with_epoch(
                "other-tile".into(),
                7,
                states[0].kind,
                &throw,
                TARGET,
                RATE,
            );
            assert!(states[0].take_over_tail(&mut incoming));
            assert_eq!(
                incoming.id, "dub_throw",
                "the incoming state carries the old id away"
            );
        }
        let x = if frame < 2_000 {
            sine(frame, 500.0)
        } else {
            0.0
        };
        let (left, _) = process_momentary_fx_states(&mut states, TARGET, x, x, Some(&mut retired));
        output.push(left);
    }
    assert_eq!(states.len(), 1);
    assert_eq!((states[0].id.as_str(), states[0].epoch), ("other-tile", 7));
    assert!(!states[0].releasing);
    let around = max_step(&output[19_900..20_100]);
    let ringing = max_step(&output[12_000..14_100]);
    assert!(
        around <= ringing + 1.0e-3,
        "takeover step {around} vs echo {ringing}"
    );
    assert!(
        output[24_000..26_100].iter().any(|x| x.abs() > 0.05),
        "the echoes keep ringing after the takeover"
    );
}

#[test]
fn a_held_tail_never_takes_over_and_other_types_or_targets_are_left_alone() {
    let mut states = state("wash", &params(&[]));
    let mut incoming = state("wash", &params(&[])).remove(0);
    assert!(!states[0].take_over_tail(&mut incoming), "still held");
    states[0].stop();
    let mut other_type = state("crush", &params(&[])).remove(0);
    assert!(!states[0].take_over_tail(&mut other_type));
    let kind = states[0].kind;
    let mut other_target = MomentaryFxState::new(
        "bus".into(),
        kind,
        &params(&[]),
        MomentaryFxTarget::FxBus { index: 0 },
        RATE,
    );
    assert!(!states[0].take_over_tail(&mut other_target));
    assert!(states[0].take_over_tail(&mut incoming));
}

#[test]
fn a_long_high_feedback_dub_tail_fades_out_after_its_limit() {
    let mut states = state(
        "dub_throw",
        &params(&[
            ("timePulses", 48.0),
            ("feedbackPct", 90.0),
            ("mixPct", 100.0),
            ("tempoBpm", 40.0),
        ]),
    );
    let hold = FRAMES * 4;
    let run = run(&mut states, hold, |n| {
        if n < hold {
            sine(n, 220.0)
        } else {
            0.0
        }
    });
    let after_release = run.removed_at - hold;
    assert!(
        after_release <= 11 * FRAMES + FRAMES / 10,
        "tail held a slot for {after_release} frames"
    );
    assert!(after_release >= 10 * FRAMES, "ends by fading, not early");
    let fade = &run.output[run.removed_at - FRAMES / 10..];
    assert!(
        fade.iter().all(|x| x.abs() < 0.05),
        "faded out before removal"
    );
}
