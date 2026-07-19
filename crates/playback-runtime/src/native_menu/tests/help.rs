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
pub(crate) fn specific_native_help_tsv_rows_are_self_resolvable() {
    let unresolved = crate::native_help::native_help_entries_for_tests()
        .iter()
        .filter(|entry| is_specific_contract_help_entry(entry))
        .filter(|entry| {
            let target = NativeMenuHelpTarget {
                path: entry.path.clone(),
                key: entry.key.clone(),
                kind: entry.kind.clone(),
                label: entry.title.clone(),
            };
            crate::native_help::resolve_native_help_entry(&target)
                .as_ref()
                .is_none_or(|resolved| !same_help_entry(entry, resolved))
        })
        .map(|entry| format!("{} {} {}", entry.kind, entry.key, entry.path))
        .collect::<Vec<_>>();

    assert!(
        unresolved.is_empty(),
        "help TSV rows not self-resolvable: {unresolved:#?}"
    );
}

fn is_specific_contract_help_entry(entry: &crate::native_help::NativeHelpEntry) -> bool {
    let key = entry.key.trim();
    let path = entry.path.trim();
    !(path == "*" && key.is_empty())
        && key != "action:*"
        && key != "key:*"
        && !path.contains('*')
        && !key.contains('*')
}

fn same_help_entry(
    a: &crate::native_help::NativeHelpEntry,
    b: &crate::native_help::NativeHelpEntry,
) -> bool {
    a.path == b.path
        && a.key == b.key
        && a.kind == b.kind
        && a.title == b.title
        && a.line1 == b.line1
        && a.line2 == b.line2
}

#[test]
pub(crate) fn behavior_category_groups_emit_keyed_help_targets() {
    let missing = platform_core::behavior_categories()
        .iter()
        .filter_map(|category| {
            let target = NativeMenuHelpTarget {
                path: "Menu > Build > Behavior: none".into(),
                key: format!("key:behavior.category.{}", category.id),
                kind: "group".into(),
                label: category.label.into(),
            };
            crate::native_help::resolve_native_help_entry(&target)
                .is_none()
                .then_some(category.id)
        })
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "missing category help targets: {missing:#?}"
    );
}

#[test]
pub(crate) fn every_catalog_behavior_leaf_resolves_behavior_specific_help() {
    let missing_specific = platform_core::behavior_catalog()
        .iter()
        .filter_map(|entry| {
            let target = NativeMenuHelpTarget {
                path: "Menu > Build > Behavior: none > [Human]".into(),
                key: format!("action:behavior_select:{}", entry.id),
                kind: "action".into(),
                label: entry.label.into(),
            };
            match crate::native_help::resolve_native_help_entry(&target) {
                Some(resolved) if resolved.key == target.key => None,
                Some(resolved) => Some((entry.id, resolved.key)),
                None => Some((entry.id, "<missing>".into())),
            }
        })
        .collect::<Vec<_>>();

    assert!(
        missing_specific.is_empty(),
        "catalog behavior help resolved through fallback or was missing: {missing_specific:#?}"
    );
}

#[test]
pub(crate) fn behavior_specific_help_beats_wildcard_fallback() {
    let specific = NativeMenuHelpTarget {
        path: "Menu > Build > Behavior: none > [Human]".into(),
        key: "action:behavior_select:keys".into(),
        kind: "action".into(),
        label: "keys".into(),
    };
    let fallback = NativeMenuHelpTarget {
        key: "action:behavior_select:not_real".into(),
        label: "not real".into(),
        ..specific.clone()
    };

    let specific_entry = crate::native_help::resolve_native_help_entry(&specific).unwrap();
    let fallback_entry = crate::native_help::resolve_native_help_entry(&fallback).unwrap();

    assert_eq!(specific_entry.key, "action:behavior_select:keys");
    assert_eq!(fallback_entry.key, "action:behavior_select:*");
}

#[test]
pub(crate) fn binding_picker_leaves_use_bound_parameter_help_targets() {
    let target = NativeMenuHelpTarget {
        path: "Menu > Play > XY > X Target > System > Sound > Note Length".into(),
        key: "key:sound.noteLengthMs".into(),
        kind: "number".into(),
        label: "Note Length".into(),
    };
    let entry = crate::native_help::resolve_native_help_entry(&target).unwrap();

    assert_eq!(entry.key, "key:sound.noteLengthMs");
    assert_ne!(entry.key, "action:param_bind");
}

#[test]
pub(crate) fn binding_picker_groups_resolve_explicit_group_help() {
    for key in [
        "key:binding.group.behavior_params",
        "key:binding.group.instruments",
        "key:binding.group.sound",
    ] {
        let target = NativeMenuHelpTarget {
            path: "Menu > Play > XY > X Target".into(),
            key: key.into(),
            kind: "group".into(),
            label: "group".into(),
        };
        assert!(
            crate::native_help::resolve_native_help_entry(&target).is_some(),
            "unresolved binding group {key}"
        );
    }
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
        for (label, value) in [("line1", *line1), ("line2", *line2)] {
            if value.chars().count() > 150 {
                problems.push(format!(
                    "line {} {label} is too long to wrap cleanly in the help popup",
                    line_number + 1
                ));
            }
        }
        for forbidden in [
            "opens this submenu",
            "shows related settings",
            "runs this command",
            "adjusts a numeric value",
            "selects one option from a list",
            "edits text for this field",
            "no help text is available",
            "see above",
            "see below",
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
