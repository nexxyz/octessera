use super::*;

#[test]
fn native_menu_help_targets_resolve_to_specific_tsv_rows() {
    let mut targets = Vec::new();
    let mut missing = Vec::new();
    for config in representative_help_configs() {
        let menu = NativeMenuModel::new(config);
        targets.extend(
            menu.help_targets()
                .into_iter()
                .filter(|target| target.kind != "action" || !target.key.is_empty()),
        );
    }
    targets.sort_by(|a, b| (&a.kind, &a.key, &a.path).cmp(&(&b.kind, &b.key, &b.path)));
    targets.dedup_by(|a, b| a.kind == b.kind && a.key == b.key && a.path == b.path);
    missing.extend(
        targets
            .into_iter()
            .filter(|target| crate::native_help::resolve_native_help_entry(target).is_none())
            .map(|target| format!("{} {} {}", target.kind, target.key, target.path)),
    );
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "missing help entries: {missing:#?}");
}

#[test]
fn native_menu_group_help_rows_match_current_paths() {
    let stale = crate::native_help::native_help_entries_for_tests()
        .iter()
        .filter(|entry| {
            entry.path.contains("Choose Sample")
                || entry.path.contains("Instrument * > S* Browse")
                || entry.path.contains("Instrument * > Sample Slot")
                || entry.path.contains("Instrument * > Assign")
                || entry.path.contains("Instrument * > Velocity Levels")
                || entry.path.contains("Instrument * > Level ")
                || entry.path.contains("Instrument * > Base Velocity")
                || entry.path.contains("Instrument * > Volume")
                || entry.path.contains("Instrument * > Filter")
                || entry.path.contains("Volume > Envelope")
                || entry.path.contains("Filter > Envelope")
        })
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    assert!(
        stale.is_empty(),
        "stale renamed group help paths: {stale:#?}"
    );
}

#[test]
fn populated_sample_browser_help_uses_actual_sample_action_keys() {
    let config = representative_help_configs()
        .into_iter()
        .find(|config| config.sample_browser.is_some())
        .expect("sample browser config");
    let menu = NativeMenuModel::new(config);
    let keys = menu
        .help_targets()
        .into_iter()
        .filter(|target| target.path.contains("S1 Browse"))
        .map(|target| target.key)
        .collect::<Vec<_>>();

    assert!(keys.iter().any(|key| key == "action:sample.up"));
    assert!(keys.iter().any(|key| key == "action:sample.enter"));
    assert!(keys.iter().any(|key| key == "action:sample.pick"));
}
