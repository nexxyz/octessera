use super::*;

#[test]
fn momentary_stutter_repeats_initial_capture() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "a".to_string(),
        "stutter".to_string(),
        BTreeMap::from([
            ("rateHz".to_string(), json!(30.0)),
            ("depthPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    let segment_len = (48_000.0 / 30.0) as usize;
    let ramp_len = ((48_000.0 * 0.002) as usize).min(segment_len / 4).max(1);

    let mut captured = Vec::new();
    for _ in 0..segment_len {
        captured.push(engine.next_sample());
    }

    let (_, _, write, ready, _) = engine.stutter_buf_for_id("a").unwrap();
    assert!(ready, "stutter should be ready after capture");
    assert_eq!(write, 0, "stutter write should be 0 after capture");

    for _ in 0..ramp_len {
        let _ = engine.next_sample();
    }

    let mut block_a = Vec::new();
    for _ in 0..segment_len {
        block_a.push(engine.next_sample());
    }
    let mut block_b = Vec::new();
    for _ in 0..segment_len {
        block_b.push(engine.next_sample());
    }

    for (i, (x, y)) in block_a.iter().zip(block_b.iter()).enumerate() {
        let diff = (x - y).abs();
        assert!(
            diff < 1.0e-6,
            "stutter loop mismatch at index {i}: a={x} b={y} diff={diff}"
        );
    }
}

#[test]
fn momentary_stutter_stop_restores_normal_output() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "a".to_string(),
        "stutter".to_string(),
        BTreeMap::from([
            ("rateHz".to_string(), json!(12.0)),
            ("depthPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    let segment_len = (48_000.0 / 12.0) as usize;
    let ramp_len = ((48_000.0 * 0.002) as usize).min(segment_len / 4).max(1);
    for _ in 0..segment_len + ramp_len + 512 {
        let _ = engine.next_sample();
    }

    engine.momentary_fx_stop("a");
    let mut released_sum = 0.0_f32;
    for _ in 0..1024 {
        released_sum += engine.next_sample().abs();
    }
    assert!(
        released_sum > 0.1,
        "stutter stop should restore audio output"
    );
}

#[test]
fn momentary_stutter_update_resets_segment_state() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "a".to_string(),
        "stutter".to_string(),
        BTreeMap::from([
            ("rateHz".to_string(), json!(30.0)),
            ("depthPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    for _ in 0..256 {
        let _ = engine.next_sample();
    }

    let params = BTreeMap::from([
        ("rateHz".to_string(), json!(12.0)),
        ("depthPct".to_string(), json!(100.0)),
    ]);
    engine.momentary_fx_update("a", &params);

    let (_, _, write, ready, ramp_pos) = engine.stutter_buf_for_id("a").unwrap();
    assert_eq!(write, 0);
    assert!(!ready);
    assert_eq!(ramp_pos, 0);
}

#[test]
fn momentary_freeze_injection_creates_sustained_tail() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 127, 10_000);

    engine.momentary_fx_start(
        "f".to_string(),
        "freeze".to_string(),
        BTreeMap::from([("mixPct".to_string(), json!(100.0))]),
        MomentaryFxTarget::Global,
    );

    let inject_samples = 48_000 * FREEZE_INJECT_MS / 1000 + 128;
    for _ in 0..inject_samples {
        let _ = engine.next_sample();
    }

    engine.note_off(0, 60);
    for _ in 0..2048 {
        let _ = engine.next_sample();
    }

    let mut sum = 0.0_f32;
    for _ in 0..2048 {
        sum += engine.next_sample().abs();
    }
    assert!(
        sum > 0.0,
        "freeze should sustain reverb tail after note release: {sum}"
    );
    assert!(sum.is_finite(), "freeze output should be finite");
}

#[test]
fn momentary_freeze_on_silence_stays_quiet() {
    let mut engine = SynthEngine::new(48_000);
    engine.momentary_fx_start(
        "f".to_string(),
        "freeze".to_string(),
        BTreeMap::from([("mixPct".to_string(), json!(100.0))]),
        MomentaryFxTarget::Global,
    );

    let inject_samples = 48_000 * FREEZE_INJECT_MS / 1000 + 128;
    for _ in 0..inject_samples {
        let _ = engine.next_sample();
    }

    engine.note_on(0, 60, 120, 1_000);
    let mut sum = 0.0_f32;
    for _ in 0..2048 {
        sum += engine.next_sample().abs();
    }
    assert!(
        sum < 1.0e-6,
        "freeze should not pass live audio through after injection window: {sum}"
    );
}

#[test]
fn momentary_freeze_release_fades_then_removes() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "f".to_string(),
        "freeze".to_string(),
        BTreeMap::from([
            ("mixPct".to_string(), json!(100.0)),
            ("releaseMs".to_string(), json!(10.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    let inject_samples = 48_000 * FREEZE_INJECT_MS / 1000 + 128;
    for _ in 0..inject_samples {
        let _ = engine.next_sample();
    }

    engine.momentary_fx_stop("f");

    let mut release_sum = 0.0_f32;
    for _ in 0..(10 * 48_000 / 1000 + 64) {
        release_sum += engine.next_sample().abs();
    }

    let mut after_sum = 0.0_f32;
    for _ in 0..512 {
        after_sum += engine.next_sample().abs();
    }

    assert!(release_sum > 0.0, "release tail should produce audio");
    assert!(
        after_sum > 0.1,
        "freeze stop should restore normal audio output: {after_sum}"
    );
}

#[test]
fn momentary_freeze_release_scales_wet_down_and_dry_up() {
    let target = MomentaryFxTarget::Global;
    let mut wet = SynthEngine::new(48_000);
    let mut releasing = SynthEngine::new(48_000);
    wet.momentary_fx_start(
        "f".to_string(),
        "freeze".to_string(),
        BTreeMap::from([("mixPct".to_string(), json!(100.0))]),
        target,
    );
    releasing.momentary_fx_start(
        "f".to_string(),
        "freeze".to_string(),
        BTreeMap::from([
            ("mixPct".to_string(), json!(75.0)),
            ("releaseMs".to_string(), json!(1.0)),
        ]),
        target,
    );
    for frame in 0..6_000 {
        let input = (0.2 + (frame % 13) as f32 * 0.01, -0.35);
        let _ = wet.process_momentary_frame_for_test(target, input.0, input.1);
        let _ = releasing.process_momentary_frame_for_test(target, input.0, input.1);
    }
    releasing.momentary_fx_stop("f");

    let mut previous_effective = 1.0_f32;
    let mut previous_dry = 0.0_f32;
    for release_pos in 0..7 {
        let input = (0.17, -0.31);
        let wet_output = wet.process_momentary_frame_for_test(target, input.0, input.1);
        let output = releasing.process_momentary_frame_for_test(target, input.0, input.1);
        let effective_mix = 0.75 * (48 - release_pos) as f32 / 48.0;
        assert!(effective_mix < previous_effective);
        assert!(1.0 - effective_mix > previous_dry);
        assert!(
            (output.0 - (input.0 * (1.0 - effective_mix) + wet_output.0 * effective_mix)).abs()
                < 1e-6
        );
        assert!(
            (output.1 - (input.1 * (1.0 - effective_mix) + wet_output.1 * effective_mix)).abs()
                < 1e-6
        );
        previous_effective = effective_mix;
        previous_dry = 1.0 - effective_mix;
    }
}

#[test]
fn momentary_freeze_update_after_activation_preserves_progress_and_retrigger_resets() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 10_000);
    engine.momentary_fx_start(
        "f".to_string(),
        "freeze".to_string(),
        BTreeMap::from([("mixPct".to_string(), json!(100.0))]),
        MomentaryFxTarget::Global,
    );
    let ready = engine.freeze_state_probe("f").unwrap().4 as usize;
    for _ in 0..ready + 16 {
        let _ = engine.next_sample();
    }
    let before = engine.freeze_state_probe("f").unwrap();
    assert!(
        before.5 > 0,
        "activation should have started after readiness"
    );
    engine.momentary_fx_update("f", &BTreeMap::from([("mixPct".to_string(), json!(50.0))]));
    let after = engine.freeze_state_probe("f").unwrap();
    assert_eq!(
        after, before,
        "update must preserve freeze buffers and progress"
    );

    engine.momentary_fx_start(
        "f".to_string(),
        "freeze".to_string(),
        BTreeMap::from([("mixPct".to_string(), json!(100.0))]),
        MomentaryFxTarget::Global,
    );
    let retriggered = engine.freeze_state_probe("f").unwrap();
    assert!(retriggered
        .0
        .iter()
        .all(|buffer| buffer.iter().all(|sample| *sample == 0.0)));
    assert_eq!(retriggered.1, [0; 4]);
    assert_eq!(retriggered.2, [0.0; 4]);
    assert_eq!(retriggered.3, 0);
    assert_eq!(retriggered.5, 0);
}
