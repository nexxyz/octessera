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

    engine.momentary_fx_update(
        "a",
        BTreeMap::from([
            ("rateHz".to_string(), json!(12.0)),
            ("depthPct".to_string(), json!(100.0)),
        ]),
    );

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
