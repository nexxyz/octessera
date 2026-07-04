use super::*;

#[test]
pub(crate) fn static_navigation_memory_ignores_dynamic_preset_lists() {
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
pub(crate) fn static_navigation_memory_does_not_affect_focus_item_key() {
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
pub(crate) fn entering_l1_selects_active_part_row() {
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
pub(crate) fn entering_l2_selects_active_part_row_after_global_rows() {
    let mut menu = NativeMenuModel::new(NativeMenuConfig {
        active_part_index: 2,
        ..config()
    });
    menu.turn(1);
    let _ = menu.press();
    menu.state.cursor = 6;
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L2: Sense");
    assert_eq!(snapshot.selected_row, Some(3));
    assert_eq!(snapshot.lines[3], "> P3: life");
}

#[test]
pub(crate) fn l2_starts_with_global_rows_and_part_rows_are_enterable() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(1);
    let _ = menu.press();
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L2: Sense");
    assert_eq!(snapshot.lines[0], "> BPM 120");
    assert_eq!(snapshot.lines[1], "  Swing % 0%");
    assert_eq!(snapshot.lines[2], "  Aux Mappings");
    menu.turn(1);
    menu.turn(1);
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
pub(crate) fn aux_mappings_follow_platform_aux_encoder_count() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(1);
    let _ = menu.press();
    menu.state.cursor = 2;
    let _ = menu.press();
    let snapshot = menu.snapshot();

    assert!(snapshot.lines.iter().any(|line| line.contains("Aux 1")));
    assert!(snapshot.lines.iter().any(|line| line.contains("Aux 2")));
    assert!(snapshot.lines.iter().any(|line| line.contains("Aux 3")));
    assert!(!snapshot.lines.iter().any(|line| line.contains("Aux 4")));
}

#[test]
pub(crate) fn auto_map_toggle_lives_under_system_ui_not_aux_mappings() {
    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![1, 2];
    let aux_snapshot = menu.snapshot();
    assert!(!aux_snapshot
        .lines
        .iter()
        .any(|line| line.contains("Auto Map")));

    menu.state.stack = vec![5, 5];
    let ui_snapshot = menu.snapshot();
    assert!(ui_snapshot
        .lines
        .iter()
        .any(|line| line.contains("Auto Map")));
}

#[test]
pub(crate) fn snapshot_scrolls_to_keep_selected_row_visible() {
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
pub(crate) fn root_matches_current_canonical_menu_without_playback_group() {
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
