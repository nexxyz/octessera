use super::*;

fn curve_of(menu: &NativeMenuModel, key: &str) -> NumberCurve {
    match menu
        .item_for_key(key)
        .unwrap_or_else(|| panic!("{key}"))
        .value
    {
        NativeMenuValue::Number { curve, .. } => curve,
        ref other => panic!("{key} is not numeric: {other:?}"),
    }
}

#[test]
fn times_frequencies_and_rates_get_curved_encoder_steps() {
    let mut cfg = config();
    cfg.instrument_types[0] = "fm".into();
    let menu = NativeMenuModel::new(cfg);
    let time = NumberCurve::Time { units_per_ms: 1 };
    assert_eq!(curve_of(&menu, "instruments.0.fm.ampEnv.attackMs"), time);
    assert_eq!(curve_of(&menu, "instruments.0.fm.indexEnv.releaseMs"), time);
    assert_eq!(curve_of(&menu, "sound.noteLengthMs"), time);
    assert_eq!(
        curve_of(&menu, "instruments.0.fm.filter.cutoffHz"),
        NumberCurve::Linear
    );
    assert_eq!(curve_of(&menu, "play.xy.smoothingMs"), NumberCurve::Linear);
    assert_eq!(
        curve_of(&menu, "instruments.0.fm.amp.gainPct"),
        NumberCurve::Linear
    );
}
