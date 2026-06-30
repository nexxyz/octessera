use super::*;
use std::collections::HashSet;

const MENU_HELP_TSV: &str = include_str!("../../../../../resources/menu-help-texts.tsv");

#[test]
pub(crate) fn native_menu_help_targets_resolve_to_specific_tsv_rows() {
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
pub(crate) fn native_menu_group_help_rows_match_current_paths() {
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
pub(crate) fn populated_sample_browser_help_uses_actual_sample_action_keys() {
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

#[test]
pub(crate) fn menu_help_tsv_rows_have_unique_ids_and_specific_text() {
    let mut ids = HashSet::new();
    let mut problems = Vec::new();

    for (line_number, line) in MENU_HELP_TSV.lines().enumerate().skip(1) {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let cols = line.split('\t').collect::<Vec<_>>();
        if cols.len() != 7 {
            problems.push(format!(
                "line {} has {} columns",
                line_number + 1,
                cols.len()
            ));
            continue;
        }
        let [id, _path, _key, _kind, title, line1, line2] = cols.as_slice() else {
            unreachable!();
        };
        if !ids.insert((*id).to_string()) {
            problems.push(format!("duplicate id {id}"));
        }
        if id.trim().is_empty() || title.trim().is_empty() {
            problems.push(format!("line {} has empty id/title", line_number + 1));
        }
        if line1.trim().is_empty() && line2.trim().is_empty() {
            problems.push(format!("line {} has no detail text", line_number + 1));
        }
        for forbidden in [
            "opens this submenu",
            "shows related settings",
            "runs this command",
            "adjusts a numeric value",
            "selects one option from a list",
            "edits text for this field",
            "no help text is available",
        ] {
            if format!("{title} {line1} {line2}")
                .to_lowercase()
                .contains(forbidden)
            {
                problems.push(format!("line {} uses generic help text", line_number + 1));
            }
        }
    }

    assert!(problems.is_empty(), "help TSV problems: {problems:#?}");
}

#[test]
pub(crate) fn menu_help_tsv_lines_stay_concise() {
    let mut long = Vec::new();
    for (line_number, line) in MENU_HELP_TSV.lines().enumerate().skip(1) {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let cols = line.split('\t').collect::<Vec<_>>();
        if cols.len() != 7 {
            continue;
        }
        for (label, value, limit) in [
            ("title", cols[4], 28usize),
            ("line1", cols[5], 150usize),
            ("line2", cols[6], 150usize),
        ] {
            if value.chars().count() > limit {
                long.push(format!(
                    "line {} {label} has {} chars: {}",
                    line_number + 1,
                    value.chars().count(),
                    value
                ));
            }
        }
    }

    assert!(long.is_empty(), "overlong help TSV fields: {long:#?}");
}
