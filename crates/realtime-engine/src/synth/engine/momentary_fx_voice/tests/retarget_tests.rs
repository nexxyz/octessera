use super::*;

/// Holds `fx_type` at its first target, then moves it to `next` and records the cutoff every
/// frame through the glide, the release and until the effect removes itself.
fn retarget_cutoffs(
    fx_type: &str,
    first: &BTreeMap<String, Value>,
    next: &BTreeMap<String, Value>,
    glide: usize,
) -> (f32, Vec<f32>, Vec<f32>) {
    let cutoff = |states: &[MomentaryFxState]| match &states[0].voice {
        MomentaryFxVoice::FilterSweep(voice) => voice.cutoff(),
        MomentaryFxVoice::HpRiser(voice) => voice.cutoff(),
        _ => panic!("sweep voice"),
    };
    let mut retired: Retired = std::array::from_fn(|_| None);
    let mut states = state(fx_type, first);
    for _ in 0..FRAMES / 2 {
        let _ = process_momentary_fx_states(&mut states, TARGET, 0.1, 0.1, Some(&mut retired));
    }
    let held = cutoff(&states);
    let update = prepare_voice_update(states[0].kind, 0, next, RATE);
    let _ = states[0].voice.apply_update(update);
    let mut gliding = vec![held];
    for _ in 0..glide {
        let _ = process_momentary_fx_states(&mut states, TARGET, 0.1, 0.1, Some(&mut retired));
        gliding.push(cutoff(&states));
    }
    states[0].stop();
    let mut releasing = Vec::new();
    while !states.is_empty() {
        assert!(releasing.len() < FRAMES, "effect never ended");
        releasing.push(cutoff(&states));
        let _ = process_momentary_fx_states(&mut states, TARGET, 0.1, 0.1, Some(&mut retired));
    }
    (held, gliding, releasing)
}

#[test]
fn filter_sweep_retarget_glides_in_octaves_from_the_current_cutoff_and_still_releases_open() {
    let first = params(&[
        ("cutoffPct", 0.0),
        ("sweepInMs", 100.0),
        ("sweepOutMs", 100.0),
    ]);
    let next = params(&[
        ("cutoffPct", 50.0),
        ("sweepInMs", 200.0),
        ("sweepOutMs", 100.0),
    ]);
    let glide = 9_600;
    let (held, gliding, releasing) = retarget_cutoffs("filter_sweep", &first, &next, glide);
    let octaves = |hz: &[f32]| hz.iter().map(|hz| hz.log2()).collect::<Vec<_>>();

    assert!(
        (held - 120.0).abs() < 0.5,
        "held at the first target {held}"
    );
    assert!(max_step(&octaves(&gliding)) < 0.001, "no jump");
    let halfway = gliding[glide / 2];
    let geometric_mean = (120.0_f32 * 4_120.0).sqrt();
    assert!(
        (halfway - geometric_mean).abs() < 2.0,
        "even in octaves {halfway}"
    );
    assert!(
        (gliding[glide] - 4_120.0).abs() < 1.0,
        "reaches the new target"
    );
    assert!(
        max_step(&octaves(&releasing)) < 0.002,
        "release has no jump"
    );
    assert!(*releasing.last().unwrap() > 19_990.0, "release ends open");
    assert!(releasing.len() <= 4_801, "release takes Sweep Out");
}

#[test]
fn hp_riser_retarget_glides_in_octaves_from_the_current_cutoff_and_still_releases_open() {
    let first = params(&[
        ("cutoffPct", 0.0),
        ("sweepInMs", 100.0),
        ("sweepOutMs", 100.0),
    ]);
    let next = params(&[
        ("cutoffPct", 100.0),
        ("sweepInMs", 200.0),
        ("sweepOutMs", 100.0),
    ]);
    let glide = 9_600;
    let (held, gliding, releasing) = retarget_cutoffs("hp_riser", &first, &next, glide);
    let octaves = |hz: &[f32]| hz.iter().map(|hz| hz.log2()).collect::<Vec<_>>();

    assert!((held - 80.0).abs() < 0.1, "held at the first target {held}");
    assert!(max_step(&octaves(&gliding)) < 0.001, "no jump");
    let halfway = gliding[glide / 2];
    assert!((halfway - 800.0).abs() < 2.0, "even in octaves {halfway}");
    assert!(
        (gliding[glide] - 8_000.0).abs() < 1.0,
        "reaches the new target"
    );
    assert!(
        max_step(&octaves(&releasing)) < 0.002,
        "release has no jump"
    );
    assert!(*releasing.last().unwrap() < 20.1, "release ends open");
    assert!(releasing.len() <= 4_801, "release takes Sweep Out");
}
