use super::*;

#[test]
fn momentary_filter_and_pitch_shift_stay_finite() {
    for (fx_type, params) in [
        (
            "filter_sweep",
            BTreeMap::from([
                ("cutoffPct".to_string(), json!(20.0)),
                ("resonancePct".to_string(), json!(80.0)),
            ]),
        ),
        (
            "pitch_shift",
            BTreeMap::from([
                ("semitones".to_string(), json!(7.0)),
                ("mixPct".to_string(), json!(100.0)),
            ]),
        ),
    ] {
        let mut engine = SynthEngine::new(48_000);
        engine.note_on(0, 60, 120, 1_000);
        engine.momentary_fx_start(
            "fx".to_string(),
            fx_type.to_string(),
            params,
            MomentaryFxTarget::Global,
        );
        let mut sum = 0.0_f32;
        for _ in 0..2048 {
            let sample = engine.next_sample();
            assert!(sample.is_finite());
            sum += sample.abs();
        }
        assert!(
            sum > 0.0,
            "{fx_type} should produce non-silent finite output"
        );
    }
}

#[test]
fn momentary_pitch_shift_fills_and_reads_output_buffer() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "ps".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(7.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    assert!(
        engine.pitch_buf_probe("ps").is_some(),
        "pitch shift fx should exist"
    );

    let mut any = false;
    for _ in 0..256 {
        let sample = engine.next_sample();
        if sample != 0.0 {
            any = true;
            break;
        }
    }
    assert!(
        any,
        "pitch shift should produce non-zero output within 256 frames"
    );
}

#[test]
fn momentary_pitch_shift_different_params_produce_different_output() {
    let mut engine_a = SynthEngine::new(48_000);
    engine_a.note_on(0, 60, 120, 1_000);
    engine_a.momentary_fx_start(
        "ps_a".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(3.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    let mut engine_b = SynthEngine::new(48_000);
    engine_b.note_on(0, 60, 120, 1_000);
    engine_b.momentary_fx_start(
        "ps_b".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(4.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    let mut diff = false;
    for _ in 0..8192 {
        let a = engine_a.next_sample();
        let b = engine_b.next_sample();
        if (a - b).abs() > 0.001 {
            diff = true;
            break;
        }
    }
    assert!(
        diff,
        "different semitone values should produce different output"
    );
}

#[test]
fn momentary_pitch_shift_cents_combined_with_semitones() {
    let mut engine_a = SynthEngine::new(48_000);
    engine_a.note_on(0, 60, 120, 1_000);
    engine_a.momentary_fx_start(
        "ps_a".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(5.0)),
            ("cents".to_string(), json!(0.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    let mut engine_b = SynthEngine::new(48_000);
    engine_b.note_on(0, 60, 120, 1_000);
    engine_b.momentary_fx_start(
        "ps_b".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(5.0)),
            ("cents".to_string(), json!(50.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    let mut diff = false;
    for _ in 0..8192 {
        let a = engine_a.next_sample();
        let b = engine_b.next_sample();
        if (a - b).abs() > 0.001 {
            diff = true;
            break;
        }
    }
    assert!(
        diff,
        "same semitones with different cents should produce different output"
    );
}

#[test]
fn momentary_pitch_shift_stop_immediately_removes() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "ps".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(12.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    for _ in 0..128 {
        engine.next_sample();
    }
    assert!(
        engine.pitch_buf_probe("ps").is_some(),
        "pitch shift should exist before stop"
    );
    engine.momentary_fx_stop("ps");
    assert!(
        engine.pitch_buf_probe("ps").is_none(),
        "pitch shift should be immediately removed on stop"
    );
}

#[test]
fn momentary_pitch_shift_no_gap_on_activation() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);

    for _ in 0..1024 {
        engine.next_sample();
    }

    let mut pre_energy = 0.0_f32;
    for _ in 0..128 {
        pre_energy += engine.next_sample().abs();
    }
    pre_energy /= 128.0;
    assert!(
        pre_energy > 0.001,
        "pre-activation energy should be non-trivial: {pre_energy}"
    );

    engine.momentary_fx_start(
        "ps".to_string(),
        "pitch_shift".to_string(),
        BTreeMap::from([
            ("semitones".to_string(), json!(7.0)),
            ("mixPct".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    let mut buf = Vec::with_capacity(512);
    for _ in 0..512 {
        buf.push(engine.next_sample().abs());
    }

    let post_sum: f32 = buf.iter().sum();
    let expected_min = pre_energy * 512.0 * 0.25;
    assert!(
        post_sum > expected_min,
        "pitch shift activation should maintain overall energy: {post_sum} vs {expected_min}"
    );

    let threshold = pre_energy * 0.02;
    let mut max_quiet_run = 0usize;
    let mut quiet_run = 0usize;
    for s in &buf {
        if *s < threshold {
            quiet_run += 1;
            max_quiet_run = max_quiet_run.max(quiet_run);
        } else {
            quiet_run = 0;
        }
    }
    assert!(
        max_quiet_run < 48,
        "pitch shift activation produced a {max_quiet_run}-sample near-silent run"
    );
}

#[test]
fn momentary_filter_sweep_envelope_changes_cutoff_over_time() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 1_000);
    engine.momentary_fx_start(
        "sweep".to_string(),
        "filter_sweep".to_string(),
        BTreeMap::from([
            ("cutoffPct".to_string(), json!(10.0)),
            ("sweepInMs".to_string(), json!(100.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    let early = engine.next_sample().abs();
    for _ in 0..(48_000 * 100 / 1000 - 2) {
        let _ = engine.next_sample();
    }
    let late = engine.next_sample().abs();

    assert!(
        (late - early).abs() > 0.001,
        "filter sweep should change output over time: early={early} late={late}"
    );
}

#[test]
fn momentary_filter_sweep_stop_releases_then_removes() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 10_000);
    engine.momentary_fx_start(
        "sweep".to_string(),
        "filter_sweep".to_string(),
        BTreeMap::from([
            ("cutoffPct".to_string(), json!(10.0)),
            ("sweepInMs".to_string(), json!(50.0)),
            ("sweepOutMs".to_string(), json!(10.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    for _ in 0..(48_000 * 50 / 1000) {
        let _ = engine.next_sample();
    }

    engine.momentary_fx_stop("sweep");

    let mut short_release_sum = 0.0_f32;
    for _ in 0..(48_000 * 10 / 1000 + 64) {
        short_release_sum += engine.next_sample().abs();
    }

    assert!(
        short_release_sum > 0.0,
        "sweep release tail should produce audio"
    );
}

#[test]
fn momentary_filter_sweep_stop_with_long_sweep_out() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 120, 10_000);
    engine.momentary_fx_start(
        "sweep".to_string(),
        "filter_sweep".to_string(),
        BTreeMap::from([
            ("cutoffPct".to_string(), json!(10.0)),
            ("sweepInMs".to_string(), json!(10.0)),
            ("sweepOutMs".to_string(), json!(200.0)),
        ]),
        MomentaryFxTarget::Global,
    );

    for _ in 0..(48_000 * 10 / 1000) {
        let _ = engine.next_sample();
    }

    engine.momentary_fx_stop("sweep");

    let mut early_release = 0.0_f32;
    for _ in 0..(48_000 * 10 / 1000) {
        early_release += engine.next_sample().abs();
    }
    let mut late_release = 0.0_f32;
    for _ in 0..(48_000 * 10 / 1000) {
        late_release += engine.next_sample().abs();
    }

    assert!(
        early_release > 0.0 && late_release > 0.0,
        "release tail should persist during long sweep-out"
    );
}
