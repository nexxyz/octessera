use super::*;

#[test]
fn restarting_momentary_fx_moves_it_to_end() {
    let mut engine = SynthEngine::new(44_100);
    engine.momentary_fx_start(
        "a".into(),
        "stutter".into(),
        BTreeMap::new(),
        MomentaryFxTarget::Global,
    );
    engine.momentary_fx_start(
        "b".into(),
        "freeze".into(),
        BTreeMap::new(),
        MomentaryFxTarget::Global,
    );
    engine.momentary_fx_start(
        "a".into(),
        "filter_sweep".into(),
        BTreeMap::new(),
        MomentaryFxTarget::Global,
    );

    let ids = engine
        .momentary_fx
        .iter()
        .map(|fx| fx.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["b", "a"]);
    assert!(matches!(
        engine.momentary_fx[1].kind,
        MomentaryFxKind::FilterSweep
    ));
}
