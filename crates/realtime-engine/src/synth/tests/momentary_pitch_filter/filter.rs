use super::super::*;

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
