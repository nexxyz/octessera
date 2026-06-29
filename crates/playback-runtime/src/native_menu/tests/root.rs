use super::*;

#[test]
fn root_snapshot_includes_l4_separator_and_system() {
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
fn rebuild_preserves_navigation_state() {
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
fn keyed_selectors_prefer_current_row_when_keys_repeat() {
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
fn navigation_skips_separator_rows_when_turning() {
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
fn system_submenu_uses_abbreviated_path_and_section_colors() {
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
            "  Controls"
        ]
    );
    assert_eq!(
        snapshot.colors,
        vec![0xB50D, 0xB50D, 0xB50D, 0xB50D, 0xB50D, 0xB50D, 0xB50D]
    );
    assert_eq!(snapshot.selected_row, Some(0));
}

#[test]
fn system_controls_screen_uses_read_only_info_rows() {
    let mut menu = NativeMenuModel::new(config());
    for _ in 0..5 {
        menu.turn(1);
    }
    let _ = menu.press();
    menu.state.cursor = 6;
    let _ = menu.press();

    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "SYS/Controls");
    assert_eq!(snapshot.lines[0], "> Help: Sh+Fn+Main");
    assert!(snapshot.lines.iter().any(|line| line == "  Back: Back"));
    assert!(snapshot.lines.iter().all(|line| !line.contains('!')));
    assert!(snapshot.selected_action.is_none());
    assert!(snapshot.line_actions.iter().all(Option::is_none));

    let stack = menu.state.stack.clone();
    let cursor = menu.state.cursor;
    assert!(menu.press().is_none());
    assert_eq!(menu.state.stack, stack);
    assert_eq!(menu.state.cursor, cursor);
}

#[test]
fn static_navigation_memory_restores_allowed_system_groups() {
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
fn static_navigation_memory_clears_on_rebuild() {
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
fn static_navigation_memory_back_while_editing_stays_in_group() {
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

#[test]
fn static_navigation_memory_ignores_dynamic_preset_lists() {
    let mut cfg = config();
    cfg.preset_names = vec!["One".into(), "Two".into()];
    let mut menu = NativeMenuModel::new(cfg);
    menu.state.stack = vec![5, 0, 0, 2];
    menu.state.cursor = 1;
    assert_eq!(menu.current_label(), Some("Two"));
    menu.back();
    assert_eq!(menu.current_label(), Some("Load"));

    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("One"));
}

#[test]
fn static_navigation_memory_does_not_affect_focus_item_key() {
    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![5, 3];
    menu.state.cursor = 2;
    menu.back();
    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("Velocity Scale"));

    assert!(menu.focus_item_key("masterVolume"));
    assert_eq!(menu.current_key(), Some("masterVolume"));
    assert_eq!(menu.current_label(), Some("Master Vol"));
}

#[test]
fn entering_l1_selects_active_part_row() {
    let mut menu = NativeMenuModel::new(NativeMenuConfig {
        active_part_index: 2,
        ..config()
    });
    let _ = menu.press();
    menu.state.cursor = 2;
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L1: Life");
    assert_eq!(snapshot.selected_row, Some(2));
    assert_eq!(snapshot.lines[2], "> P3: life");
}

#[test]
fn entering_l2_selects_active_part_row_after_global_rows() {
    let mut menu = NativeMenuModel::new(NativeMenuConfig {
        active_part_index: 2,
        ..config()
    });
    menu.turn(1);
    let _ = menu.press();
    menu.state.cursor = 4;
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L2: Sense");
    assert_eq!(snapshot.selected_row, Some(3));
    assert_eq!(snapshot.lines[3], "> P3: life");
}

#[test]
fn l2_starts_with_global_rows_and_part_rows_are_enterable() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(1);
    let _ = menu.press();
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L2: Sense");
    assert_eq!(snapshot.lines[0], "> Aux Mappings");
    assert_eq!(snapshot.lines[1], "  Events when paused");
    assert_eq!(snapshot.lines[2], "  P1: life");
    menu.turn(1);
    menu.turn(1);
    let _ = menu.press();
    let part_snapshot = menu.snapshot();
    assert_eq!(part_snapshot.path, "L2: Sense/P1: life");
    assert!(part_snapshot.lines.iter().any(|line| line == "> Scanning"));
    assert!(part_snapshot.lines.iter().any(|line| line == "  Events"));
    assert!(part_snapshot
        .lines
        .iter()
        .any(|line| line == "  Note Mapping"));
}

#[test]
fn aux_mappings_follow_platform_aux_encoder_count() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(1);
    let _ = menu.press();
    let _ = menu.press();
    let snapshot = menu.snapshot();

    assert!(snapshot.lines.iter().any(|line| line.trim() == "Aux 1"));
    assert!(snapshot.lines.iter().any(|line| line.trim() == "Aux 2"));
    assert!(snapshot.lines.iter().any(|line| line.trim() == "Aux 3"));
    assert!(!snapshot.lines.iter().any(|line| line.trim() == "Aux 4"));
}

#[test]
fn snapshot_scrolls_to_keep_selected_row_visible() {
    let mut menu = NativeMenuModel::new(config());
    let _ = menu.press();
    menu.state.cursor = 7;
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L1: Life");
    assert_eq!(snapshot.selected_row, Some(6));
    assert_eq!(
        snapshot.scroll.as_ref().map(|scroll| scroll.scroll_offset),
        Some(1)
    );
    assert_eq!(
        snapshot.scroll.as_ref().map(|scroll| scroll.total_rows),
        Some(8)
    );
    assert_eq!(
        snapshot.scroll.as_ref().map(|scroll| scroll.visible_rows),
        Some(7)
    );
    assert_eq!(snapshot.lines[6], "> P8: life");
    assert!(!snapshot.lines.iter().any(|line| line == "  P1: life"));
}

#[test]
fn root_matches_current_canonical_menu_without_playback_group() {
    let menu = NativeMenuModel::new(config());
    let labels = menu
        .root
        .children
        .iter()
        .filter(|item| !item.label.is_empty())
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec!["L1: Life", "L2: Sense", "L3: Voice", "L4: Dance", "System"]
    );
}
