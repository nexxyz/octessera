use super::*;

#[test]
pub(crate) fn root_snapshot_includes_l4_separator_and_system() {
    let menu = NativeMenuModel::new(config());
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "MENU");
    assert_eq!(
        snapshot.lines,
        vec![
            "> L1: Life",
            "  L2: Sense",
            "  L3: Voice",
            "  L4: Dance",
            "",
            "  System"
        ]
    );
    assert_eq!(snapshot.selected_row, Some(0));
    assert_eq!(
        snapshot.colors,
        vec![0x8ED1, 0x8D5C, 0xC59B, 0xFFFF, 0xFFFF, 0xB50D]
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
    next.behavior_id = "glider".into();
    next.l1_items[0].value = NativeMenuValue::Enum {
        options: vec!["life".into(), "glider".into(), "none".into()],
        selected: 1,
    };
    menu.rebuild(next);
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L1: Life/P1: life");
    assert_eq!(snapshot.selected_row, Some(0));
    assert!(menu.state.editing);
    assert_eq!(menu.selected_behavior().as_deref(), Some("glider"));
}

#[test]
pub(crate) fn keyed_selectors_prefer_current_row_when_keys_repeat() {
    let mut cfg = config();
    cfg.l1_items = vec![
        NativeMenuItem {
            label: "Other Dance".into(),
            key: Some("danceMode".into()),
            value: NativeMenuValue::Enum {
                options: vec!["none".into(), "fx".into()],
                selected: 0,
            },
            children: vec![],
        },
        NativeMenuItem {
            label: "Dance".into(),
            key: Some("danceMode".into()),
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
    assert_eq!(menu.selected_dance_mode().as_deref(), Some("fx"));

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
    assert_eq!(snapshot.lines[5], "> System");
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
            "> Saves",
            "  Diagnostics",
            "  Updates",
            "  Sound",
            "  MIDI",
            "  UI",
            "  !Basic Help"
        ]
    );
    assert_eq!(
        snapshot.colors,
        vec![0xB50D, 0xB50D, 0xB50D, 0xB50D, 0xB50D, 0xB50D, 0xB50D]
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
    menu.state.cursor = 6;

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
    menu.state.cursor = 3;
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
    menu.state.cursor = 3;
    let _ = menu.press();
    menu.turn(1);
    menu.turn(1);
    menu.back();
    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("Velocity Scale"));

    menu.rebuild(config());
    menu.state.stack = vec![5];
    menu.state.cursor = 3;
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
    menu.state.cursor = 3;
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
