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
pub(crate) fn configure_wifi_uses_a_stable_resolvable_action_help_key() {
    let target = representative_help_configs()
        .into_iter()
        .flat_map(|config| NativeMenuModel::new(config).help_targets())
        .find(|target| target.key == "action:system_configure_wifi")
        .expect("configure WiFi help target");
    let entry =
        crate::native_help::resolve_native_help_entry(&target).expect("configure WiFi help entry");
    assert_eq!(entry.key, "action:system_configure_wifi");
    let copy = format!("{} {}", entry.line1, entry.line2);
    assert!(copy.contains("full setup portal"));
    assert!(copy.contains("Wi-Fi"));
    assert!(copy.contains("hostname"));
    assert!(copy.contains("SSH"));
    assert!(copy.contains("login"));
}

#[test]
pub(crate) fn hdmi_help_covers_terminal_and_bars_per_cycle_copy() {
    let mut config = config();
    config.hdmi_mode = "cycle-behaviors".into();
    let menu = NativeMenuModel::new(config);

    let mode_target = menu
        .help_targets()
        .into_iter()
        .find(|target| target.key == "key:hdmi.mode")
        .expect("HDMI mode help target");
    let mode_entry =
        crate::native_help::resolve_native_help_entry(&mode_target).expect("HDMI mode help entry");
    let mode_copy = format!("{} {}", mode_entry.line1, mode_entry.line2).to_lowercase();
    for phrase in ["terminal", "none", "live-grid", "cycle-behaviors"] {
        assert!(
            mode_copy.contains(phrase),
            "HDMI mode help omitted {phrase}"
        );
    }

    let cycle_target = menu
        .help_targets()
        .into_iter()
        .find(|target| target.key == "key:hdmi.cycleMeasures")
        .expect("Bars per cycle help target");
    let cycle_entry = crate::native_help::resolve_native_help_entry(&cycle_target)
        .expect("Bars per cycle help entry");
    assert_eq!(cycle_entry.title, "Bars per cycle");
    let cycle_copy = format!("{} {}", cycle_entry.line1, cycle_entry.line2);
    assert!(cycle_copy.contains("musical bars each behavior remains shown"));
    assert!(cycle_copy.contains("before Cycle Behaviors advances"));
}

#[test]
pub(crate) fn duck_range_help_does_not_resolve_for_compressor_slots() {
    for (parameter, label, range) in [
        ("attackMs", "Attack", "1–500 ms"),
        ("releaseMs", "Release", "1–5000 ms"),
    ] {
        let duck_target = fx_bus_parameter_help_target("duck", "Duck", parameter, label);
        let duck_entry =
            crate::native_help::resolve_native_help_entry(&duck_target).expect("Duck help entry");
        assert_eq!(duck_entry.kind, "number");
        assert!(duck_entry.path.contains(&format!("Slot *: Duck > {label}")));
        assert!(
            format!("{} {}", duck_entry.line1, duck_entry.line2).contains(range),
            "Duck {parameter} help omitted {range:?}"
        );

        let compressor_target =
            fx_bus_parameter_help_target("compressor", "Compressor", parameter, label);
        let compressor_entry = crate::native_help::resolve_native_help_entry(&compressor_target)
            .expect("Compressor help entry");
        let compressor_copy = format!("{} {}", compressor_entry.line1, compressor_entry.line2);
        assert_eq!(compressor_entry.kind, "number");
        assert!(compressor_entry
            .path
            .contains(&format!("Slot *: * > {label}")));
        assert!(compressor_entry.key.is_empty());
        assert!(
            !compressor_copy.contains("Duck"),
            "target {compressor_target:?} resolved {compressor_entry:?}"
        );
        assert!(!compressor_copy.contains(range), "{compressor_entry:?}");
        assert!(
            compressor_copy.contains("this effect"),
            "{compressor_entry:?}"
        );
    }
}

fn fx_bus_parameter_help_target(
    slot_type: &str,
    slot_label: &str,
    parameter: &str,
    label: &str,
) -> NativeMenuHelpTarget {
    let mut config = config();
    config.fx_buses[0].slot1_type = slot_type.into();
    config.fx_buses[0].slot1_params = serde_json::json!({});
    NativeMenuModel::new(config)
        .help_targets()
        .into_iter()
        .find(|target| {
            target.label == label
                && target.key.starts_with("key:mixer.buses.")
                && target.key.ends_with(&format!(".params.{parameter}"))
                && target.path.contains(&format!("Slot 1: {slot_label}"))
        })
        .unwrap_or_else(|| panic!("missing {slot_type} {parameter} help target"))
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
pub(crate) fn affected_behavior_help_targets_keep_one_accurate_row() {
    let cases: &[(&str, &str, &str, &str, &[&str])] = &[
        (
            "Menu > Build > L*: * > Cell Life",
            "*",
            "key:layers.*.worlds.behaviorConfig.cellLife",
            "number",
            &["crystal growth", "dla"],
        ),
        (
            "Menu > Build > L*: * > Seed",
            "*",
            "key:layers.*.worlds.behaviorConfig.seed",
            "number",
            &["twinkle", "pattern"],
        ),
        (
            "Menu > Build > L*: * > Erosion",
            "*",
            "key:layers.*.worlds.behaviorConfig.erosionPct",
            "number",
            &["sand ripples", "rivers"],
        ),
        (
            "Menu > Build > L*: * > Diffusion",
            "*",
            "key:layers.*.worlds.behaviorConfig.diffusionPct",
            "number",
            &["ink", "reaction diffusion"],
        ),
        (
            "Menu > Build > L*: * > Growth",
            "*",
            "key:layers.*.worlds.behaviorConfig.growthPct",
            "number",
            &["coral", "vines"],
        ),
        (
            "Menu > Build > L*: * > Branch",
            "*",
            "key:layers.*.worlds.behaviorConfig.branchPct",
            "number",
            &["cracks", "vines"],
        ),
        (
            "Menu > Build > L*: * > Evaporation",
            "*",
            "key:layers.*.worlds.behaviorConfig.evaporationPct",
            "number",
            &["physarum", "rivers"],
        ),
        (
            "Menu > Shape > Instruments > Instrument * > Synth > Amp Env",
            "Menu > Shape > Instruments > Instrument * > Synth > Amp Env",
            "",
            "group",
            &["synth", "adsr"],
        ),
        (
            "Menu > Shape > Instruments > Instrument * > Synth > Filter Env",
            "Menu > Shape > Instruments > Instrument * > Synth > Filter Env",
            "",
            "group",
            &["synth", "adsr"],
        ),
    ];

    for (target_path, row_path, key, kind, phrases) in cases {
        let rows = crate::native_help::native_help_entries_for_tests()
            .iter()
            .filter(|entry| entry.path == *row_path && entry.key == *key && entry.kind == *kind)
            .count();
        assert_eq!(
            rows, 1,
            "expected one active help row for {key} at {row_path}"
        );

        let target = NativeMenuHelpTarget {
            path: (*target_path).into(),
            key: (*key).into(),
            kind: (*kind).into(),
            label: "affected target".into(),
        };
        let entry = crate::native_help::resolve_native_help_entry(&target)
            .unwrap_or_else(|| panic!("missing help for {key} at {target_path}"));
        let copy = format!("{} {}", entry.line1, entry.line2).to_lowercase();
        for phrase in *phrases {
            assert!(
                copy.contains(phrase),
                "help for {key} at {target_path} is missing {phrase:?}: {copy}"
            );
        }
    }

    let fallback = NativeMenuHelpTarget {
        path: "Menu > Build > L*: * > Range Min".into(),
        key: "key:layers.*.worlds.behaviorConfig.rangeMin".into(),
        kind: "number".into(),
        label: "Range Min".into(),
    };
    let fallback_entry = crate::native_help::resolve_native_help_entry(&fallback).unwrap();
    assert_eq!(fallback_entry.key, "key:*.rangeMin");
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
pub(crate) fn menu_help_tsv_rows_meet_resource_contract() {
    let mut ids = HashSet::new();
    let mut problems = Vec::new();

    for (line_number, line) in MENU_HELP_TSV.lines().enumerate().skip(1) {
        let row = line_number + 1;
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let cols = line.split('\t').collect::<Vec<_>>();
        if cols.len() != 7 {
            problems.push(format!("line {row} has {} columns", cols.len()));
            continue;
        }
        let (id, title, line1, line2) = (cols[0], cols[4], cols[5], cols[6]);
        if !ids.insert((*id).to_string()) {
            problems.push(format!("duplicate id {id}"));
        }
        if id.trim().is_empty() || title.trim().is_empty() {
            problems.push(format!("line {row} has empty id/title"));
        }
        if line1.trim().is_empty() && line2.trim().is_empty() {
            problems.push(format!("line {row} has no detail text"));
        }
        for (label, value, limit) in [
            ("title", title, 28usize),
            ("line1", line1, 150),
            ("line2", line2, 150),
        ] {
            if value.chars().count() > limit {
                problems.push(format!(
                    "line {} {label} has {} chars: {value}",
                    row,
                    value.chars().count()
                ));
            }
        }
        let copy = format!("{title} {line1} {line2}").to_lowercase();
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
            if copy.contains(forbidden) {
                problems.push(format!("line {row} uses generic help text"));
            }
        }
    }

    assert!(problems.is_empty(), "help TSV problems: {problems:#?}");
}
