use super::*;

#[test]
fn factory_load_lists_are_type_specific_and_emit_exact_platform_effects() {
    for (kind, effect_kind, family, choices) in [
        (
            "fm",
            "fm",
            "preset",
            vec![
                ("init", "Init"),
                ("soft_keys", "Soft Keys"),
                ("bell", "Bell"),
            ],
        ),
        (
            "pluck",
            "pluck",
            "preset",
            vec![
                ("init", "Init"),
                ("nylon", "Nylon"),
                ("steel", "Steel"),
                ("muted", "Muted"),
            ],
        ),
        (
            "sampler",
            "sample",
            "kit",
            vec![
                ("sdbkit", "SDB Kit"),
                ("synthkit", "Synth Kit"),
                ("distkit", "Dist Kit"),
            ],
        ),
        (
            "drum",
            "drum",
            "kit",
            vec![
                ("default", "Default"),
                ("tight", "Tight"),
                ("heavy", "Heavy"),
            ],
        ),
    ] {
        let mut cfg = config();
        cfg.instrument_types[0] = kind.into();
        let mut menu = NativeMenuModel::new(cfg);
        for (id, label) in choices {
            let key = format!("{effect_kind}.{family}.0.{id}");
            let effect = format!("{effect_kind}.{family}:0:{id}");
            let item = menu.item_for_key(&key).expect("factory load action");
            assert_eq!(item.label, label);
            assert!(menu.focus_item_key(&key));
            assert_eq!(
                menu.snapshot().selected_action,
                Some(NativeMenuAction::PlatformEffect(effect.clone()))
            );
            assert_eq!(
                menu.press(),
                Some(NativeMenuPressResult::Action(
                    NativeMenuAction::PlatformEffect(effect)
                ))
            );
        }
        for other in ["fm", "pluck", "sample", "drum"] {
            if other != effect_kind {
                assert!(menu
                    .item_for_key(&format!("{other}.kit.0.default"))
                    .is_none());
                assert!(menu
                    .item_for_key(&format!("{other}.preset.0.init"))
                    .is_none());
            }
        }
    }
    for kind in ["synth", "midi", "none"] {
        let mut cfg = config();
        cfg.instrument_types[0] = kind.into();
        let menu = NativeMenuModel::new(cfg);
        for key in [
            "fm.preset.0.init",
            "pluck.preset.0.init",
            "sample.kit.0.sdbkit",
            "drum.kit.0.default",
        ] {
            assert!(menu.item_for_key(key).is_none(), "{kind}: {key}");
        }
    }
}

#[test]
fn factory_load_help_has_specific_confirmation_and_selector_copy() {
    for (kind, label, help_key, selector) in [
        (
            "fm",
            "Soft Keys",
            "action:fm_preset_load",
            "key:instruments.*.fm.ratio",
        ),
        (
            "pluck",
            "Nylon",
            "action:pluck_preset_load",
            "key:instruments.*.pluck.filter.type",
        ),
        (
            "sampler",
            "SDB Kit",
            "action:sample_kit_load",
            "key:instruments.*.sample.selectedSlot",
        ),
        (
            "drum",
            "Heavy",
            "action:drum_kit_load",
            "key:instruments.*.drum.voice",
        ),
    ] {
        let mut cfg = config();
        cfg.instrument_types[0] = kind.into();
        let menu = NativeMenuModel::new(cfg);
        let targets = menu.help_targets();
        let action = targets
            .iter()
            .find(|target| {
                target.key == help_key && target.path.ends_with(&format!(" > Load > {label}"))
            })
            .expect("factory action target");
        let help =
            crate::native_help::resolve_native_help_entry(action).expect("factory action help");
        assert_eq!(help.key, help_key);
        assert!(format!("{} {}", help.line1, help.line2).contains("Cancel"));
        let group_path = action.path.rsplit_once(" > Load > ").unwrap().0;
        for path in [group_path, &format!("{group_path} > Load")] {
            let target = targets
                .iter()
                .find(|target| target.path == path && target.kind == "group")
                .expect("factory group help target");
            let entry =
                crate::native_help::resolve_native_help_entry(target).expect("factory group help");
            assert_eq!(entry.path, path.replace("Instrument 1", "Instrument *"));
        }
        let selector = targets
            .iter()
            .find(|target| target.key == selector)
            .expect("type selector help target");
        let selector_help =
            crate::native_help::resolve_native_help_entry(selector).expect("type selector help");
        assert_eq!(selector_help.kind, "enum");
    }
}

#[test]
fn sampler_kit_precedes_unchanged_sample_rows_with_or_without_velocity_levels() {
    for enabled in [false, true] {
        let mut cfg = config();
        cfg.instrument_types[0] = "sampler".into();
        cfg.instrument_sample_velocity_levels_enabled[0] = enabled;
        let menu = NativeMenuModel::new(cfg);
        let sampler = &menu.root.children[2].children[0].children[0].children[2];
        let labels = sampler
            .children
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        let mut expected = vec![
            "Kit",
            "Sample Slot",
            "(empty)",
            "S1 Browse",
            "Assign",
            "Tune",
            "Gain",
            "Base Velocity",
        ];
        if enabled {
            expected.push("Vel Levels");
        }
        expected.extend(["Vel Levels", "Filter", "Vel Sens", "Amp Env", "Filter Env"]);
        assert_eq!(labels, expected);
    }
}

#[test]
fn drum_volume_is_selected_after_seven_body_rows_without_losing_scroll() {
    let mut cfg = config();
    cfg.instrument_types[0] = "drum".into();
    let mut menu = NativeMenuModel::new(cfg);
    assert!(menu.focus_item_key("instruments.0.type"));
    assert!(menu.focus_current_group_label("Drum"));
    assert_eq!(menu.press(), Some(NativeMenuPressResult::EnteredGroup));
    assert_eq!(menu.snapshot().scroll.as_ref().unwrap().total_rows, 8);
    menu.turn(7);
    let snapshot = menu.snapshot();
    assert_eq!(menu.current_item().label, "Volume");
    assert_eq!(snapshot.lines.len(), 7);
    assert!(snapshot.lines[snapshot.selected_row.unwrap()].starts_with("> Volume"));
    assert!(snapshot.scroll.as_ref().unwrap().scroll_offset > 0);
    assert_eq!(snapshot.scroll.as_ref().unwrap().total_rows, 8);
    assert_eq!(menu.press(), Some(NativeMenuPressResult::EnteredGroup));
    assert!(menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line.contains("Gain")));
}
