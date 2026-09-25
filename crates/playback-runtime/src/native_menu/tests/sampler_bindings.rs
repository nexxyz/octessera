use super::*;

#[test]
fn sampler_picker_keeps_rendered_targets_but_not_unrendered_filter_or_envelopes() {
    let mut sampler_config = config();
    sampler_config.instrument_types[0] = "sampler".into();
    sampler_config.instrument_sample_velocity_levels_enabled[0] = true;
    let menu = NativeMenuModel::new(sampler_config);
    let picker = menu.item_for_key("aux:0:turn").expect("Aux binding picker");
    for field in [
        "tuneSemis",
        "amp.gainPct",
        "amp.velocitySensitivityPct",
        "baseVelocity",
        "filter.cutoffHz",
        "filter.resonance",
        "velocityLevels.high",
        "velocityLevels.medium",
        "velocityLevels.low",
    ] {
        let key = format!("aux:0:turn.instruments.0.sample.{field}");
        assert!(find_item_by_key(&picker, &key).is_some(), "{key}");
    }
    for field in [
        "filter.type",
        "filter.envAmountPct",
        "filter.keyTrackingPct",
        "ampEnv.attackMs",
        "ampEnv.decayMs",
        "ampEnv.sustainPct",
        "ampEnv.releaseMs",
        "filterEnv.attackMs",
        "filterEnv.decayMs",
        "filterEnv.sustainPct",
        "filterEnv.releaseMs",
    ] {
        let key = format!("instruments.0.sample.{field}");
        assert!(
            menu.item_for_key(&key).is_some(),
            "saved menu row missing: {key}"
        );
        assert!(
            find_item_by_key(&picker, &format!("aux:0:turn.{key}")).is_none(),
            "binding picker still offers {key}"
        );
    }

    let mut fm_config = config();
    fm_config.instrument_types[0] = "fm".into();
    let fm_menu = NativeMenuModel::new(fm_config);
    let fm_picker = fm_menu
        .item_for_key("aux:0:turn")
        .expect("FM binding picker");
    for field in [
        "index",
        "amp.gainPct",
        "filter.cutoffHz",
        "filter.resonance",
    ] {
        let key = format!("aux:0:turn.instruments.0.fm.{field}");
        assert!(find_item_by_key(&fm_picker, &key).is_some(), "{key}");
    }
    for field in ["ratio", "filter.type"] {
        let key = format!("aux:0:turn.instruments.0.fm.{field}");
        assert!(find_item_by_key(&fm_picker, &key).is_none(), "{key}");
    }
}
