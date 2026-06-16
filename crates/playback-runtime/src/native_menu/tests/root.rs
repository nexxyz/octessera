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
            "> L2: Sense",
            "> L3: Voice",
            "> L4: Dance",
            "",
            "> System"
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
    assert_eq!(snapshot.lines, vec!["> Saves", "> Sound", "> MIDI", "> UI"]);
    assert_eq!(snapshot.colors, vec![0xB50D, 0xB50D, 0xB50D, 0xB50D]);
    assert_eq!(snapshot.selected_row, Some(0));
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
fn entering_l2_selects_active_part_row_after_aux_mappings_group() {
    let mut menu = NativeMenuModel::new(NativeMenuConfig {
        active_part_index: 2,
        ..config()
    });
    menu.turn(1);
    let _ = menu.press();
    menu.state.cursor = 3;
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L2: Sense");
    assert_eq!(snapshot.selected_row, Some(3));
    assert_eq!(snapshot.lines[3], "> P3: life");
}

#[test]
fn l2_starts_with_aux_mappings_and_part_rows_are_enterable() {
    let mut menu = NativeMenuModel::new(config());
    menu.turn(1);
    let _ = menu.press();
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L2: Sense");
    assert_eq!(snapshot.lines[0], "> Aux Mappings");
    assert_eq!(snapshot.lines[1], "> P1: life");
    menu.turn(1);
    let _ = menu.press();
    let part_snapshot = menu.snapshot();
    assert_eq!(part_snapshot.path, "L2: Sense/P1: life");
    assert!(part_snapshot.lines.iter().any(|line| line == "> Scanning"));
    assert!(part_snapshot.lines.iter().any(|line| line == "> Events"));
    assert!(part_snapshot
        .lines
        .iter()
        .any(|line| line == "> Note Mapping"));
}

#[test]
fn snapshot_scrolls_to_keep_selected_row_visible() {
    let mut menu = NativeMenuModel::new(config());
    let _ = menu.press();
    menu.state.cursor = 7;
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "L1: Life");
    assert_eq!(snapshot.selected_row, Some(6));
    assert_eq!(snapshot.lines[6], "> P8: life");
    assert!(!snapshot.lines.iter().any(|line| line == "> P1: life"));
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
