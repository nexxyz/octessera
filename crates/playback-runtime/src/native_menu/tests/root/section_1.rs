use super::*;

#[test]
pub(crate) fn root_snapshot_includes_sparks_separator_and_system() {
    let menu = NativeMenuModel::new(config());
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "MENU");
    assert_eq!(
        snapshot.lines,
        vec![
            "> 1: Worlds >",
            "  2: Pulses >",
            "  3: Tones >",
            "  4: Sparks >",
            "",
            "  System >"
        ]
    );
    assert_eq!(snapshot.selected_row, Some(0));
    assert_eq!(
        snapshot.colors,
        vec![
            platform_core::palette::WORLDS_RGB565,
            platform_core::palette::PULSES_RGB565,
            platform_core::palette::TONES_RGB565,
            platform_core::palette::SPARKS_RGB565,
            platform_core::palette::WHITE_RGB565,
            platform_core::palette::SYSTEM_RGB565
        ]
    );
}

#[test]
pub(crate) fn rebuild_preserves_navigation_state() {
    let mut menu = NativeMenuModel::new(config());
    let _ = menu.press();
    let _ = menu.press();
    let _ = menu.press();
    menu.turn(1);
    let mut next = config();
    next.behavior_id = "brain".into();
    next.worlds_items[0].value = NativeMenuValue::Enum {
        options: vec!["life".into(), "brain".into(), "none".into()],
        selected: 1,
    };
    menu.rebuild(next);
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "W/L1: life");
    assert_eq!(snapshot.selected_row, Some(0));
    assert!(menu.state.editing);
    assert_eq!(menu.selected_behavior().as_deref(), Some("brain"));
}

#[test]
pub(crate) fn keyed_selectors_prefer_current_row_when_keys_repeat() {
    let mut cfg = config();
    cfg.worlds_items = vec![
        NativeMenuItem {
            label: "Other Sparks".into(),
            key: Some("sparksMode".into()),
            value: NativeMenuValue::Enum {
                options: vec!["none".into(), "fx".into()],
                selected: 0,
            },
            children: vec![],
        },
        NativeMenuItem {
            label: "Sparks".into(),
            key: Some("sparksMode".into()),
            value: NativeMenuValue::Enum {
                options: vec!["none".into(), "fx".into()],
                selected: 1,
            },
            children: vec![],
        },
        NativeMenuItem {
            label: "Other Sync".into(),
            key: Some("midiSyncMode".into()),
            value: NativeMenuValue::Enum {
                options: vec!["internal".into(), "external".into()],
                selected: 0,
            },
            children: vec![],
        },
        NativeMenuItem {
            label: "Sync".into(),
            key: Some("midiSyncMode".into()),
            value: NativeMenuValue::Enum {
                options: vec!["internal".into(), "external".into()],
                selected: 1,
            },
            children: vec![],
        },
    ];
    let mut menu = NativeMenuModel::new(cfg);
    menu.state.stack = vec![0, 0];

    menu.state.cursor = 1;
    assert_eq!(menu.selected_sparks_mode(), None);

    menu.state.cursor = 3;
    assert_eq!(menu.selected_sync_source(), Some(SyncSource::External));
}

#[test]
pub(crate) fn navigation_skips_separator_rows_when_turning() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(1);
    menu.turn(1);
    menu.turn(1);
    assert_eq!(menu.snapshot().selected_row, Some(3));
    menu.turn(1);
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.selected_row, Some(5));
    assert_eq!(snapshot.lines[5], "> System >");
}

#[test]
pub(crate) fn system_submenu_uses_abbreviated_path_and_section_colors() {
    let mut menu = NativeMenuModel::new(config());
    for _ in 0..5 {
        menu.turn(1);
    }
    let _ = menu.press();
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "SYS");
    assert_eq!(
        snapshot.lines,
        vec![
            "> Saves >",
            "  Updates >",
            "  Sound >",
            "  MIDI >",
            "  UI >",
            "  !Basic Help",
            "  Diagnostics >"
        ]
    );
    assert_eq!(
        snapshot.colors,
        vec![
            platform_core::palette::SYSTEM_RGB565,
            platform_core::palette::SYSTEM_RGB565,
            platform_core::palette::SYSTEM_RGB565,
            platform_core::palette::SYSTEM_RGB565,
            platform_core::palette::SYSTEM_RGB565,
            platform_core::palette::SYSTEM_RGB565,
            platform_core::palette::SYSTEM_RGB565
        ]
    );
    assert_eq!(snapshot.selected_row, Some(0));
}

#[test]
pub(crate) fn system_controls_row_is_help_action() {
    let mut menu = NativeMenuModel::new(config());
    for _ in 0..5 {
        menu.turn(1);
    }
    let _ = menu.press();
    menu.state.cursor = 5;

    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "SYS");
    assert!(snapshot.lines.iter().any(|line| line == "> !Basic Help"));
    assert!(matches!(
        snapshot.selected_action,
        Some(NativeMenuAction::PlatformEffect(ref action)) if action == "system.controlsHelp"
    ));
}

#[test]
pub(crate) fn static_navigation_memory_restores_allowed_system_groups() {
    let mut menu = NativeMenuModel::new(config());
    for _ in 0..5 {
        menu.turn(1);
    }
    let _ = menu.press();
    menu.state.cursor = 2;
    let _ = menu.press();
    assert_eq!(menu.snapshot().lines[0], "> Master Vol 100");

    menu.turn(1);
    menu.turn(1);
    assert_eq!(menu.current_label(), Some("Velocity Scale"));
    menu.back();
    assert_eq!(menu.current_label(), Some("Sound"));

    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("Velocity Scale"));
}

#[test]
pub(crate) fn static_navigation_memory_clears_on_rebuild() {
    let mut menu = NativeMenuModel::new(config());
    for _ in 0..5 {
        menu.turn(1);
    }
    let _ = menu.press();
    menu.state.cursor = 2;
    let _ = menu.press();
    menu.turn(1);
    menu.turn(1);
    menu.back();
    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("Velocity Scale"));

    menu.rebuild(config());
    menu.state.stack = vec![5];
    menu.state.cursor = 2;
    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("Master Vol"));
}

#[test]
pub(crate) fn static_navigation_memory_back_while_editing_stays_in_group() {
    let mut menu = NativeMenuModel::new(config());
    for _ in 0..5 {
        menu.turn(1);
    }
    let _ = menu.press();
    menu.state.cursor = 2;
    let _ = menu.press();
    menu.turn(1);
    assert_eq!(menu.current_label(), Some("Note Length"));

    let _ = menu.press();
    assert!(menu.state.editing);
    menu.back();
    assert!(!menu.state.editing);
    assert_eq!(menu.snapshot().path, "SYS/Sound");
    assert_eq!(menu.current_label(), Some("Note Length"));

    menu.back();
    assert_eq!(menu.current_label(), Some("Sound"));
    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("Note Length"));
}
