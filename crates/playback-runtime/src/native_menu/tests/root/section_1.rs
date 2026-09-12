use super::*;

#[test]
pub(crate) fn root_snapshot_includes_sparks_separator_and_system() {
    let menu = NativeMenuModel::new(config());
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "MENU");
    assert_eq!(
        snapshot.lines,
        vec![
            "> Build >",
            "  Link >",
            "  Shape >",
            "  Play >",
            "",
            "  System >"
        ]
    );
    assert_eq!(snapshot.selected_row, Some(0));
    assert_eq!(
        snapshot.colors,
        vec![
            platform_core::palette::GREEN_RGB565,
            platform_core::palette::RED_RGB565,
            platform_core::palette::BLUE_RGB565,
            platform_core::palette::YELLOW_RGB565,
            platform_core::palette::WHITE_RGB565,
            platform_core::palette::GRAY_RGB565
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
    assert_eq!(snapshot.path, "/B/L1: life");
    assert_eq!(snapshot.selected_row, Some(0));
    assert!(menu.state.editing);
    assert_eq!(menu.selected_behavior().as_deref(), Some("brain"));
}

#[test]
pub(crate) fn keyed_selectors_prefer_current_row_when_keys_repeat() {
    let mut cfg = config();
    cfg.worlds_items = vec![
        NativeMenuItem {
            label: "Other Play".into(),
            key: Some("sparksMode".into()),
            value: NativeMenuValue::Enum {
                options: vec!["none".into(), "fx".into()],
                selected: 0,
            },
            children: vec![],
        },
        NativeMenuItem {
            label: "Play".into(),
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
    assert_eq!(menu.current_label(), Some("Play"));
    menu.turn(1);
    let snapshot = menu.snapshot();
    let selected_row = snapshot.selected_row.expect("selected row");
    assert_eq!(snapshot.lines[selected_row], "> System >");
}

#[test]
pub(crate) fn system_tree_has_exact_order_and_platform_visibility() {
    let desktop = config();
    assert_eq!(
        system_labels(&desktop),
        vec![
            "Save Current",
            "Load Preset",
            "Master Vol",
            "Panic",
            "Sys. Info",
            "Basic Help",
            "Recording",
            "Notes",
            "MIDI",
            "Audio",
            "UI",
            "Saves",
            "Setup",
            "Reset",
            "Reboot",
            "Shutdown",
        ]
    );
    assert_eq!(
        group_labels(&desktop, "Recording"),
        vec!["Max Time", "Start Audio", "St. Audio+OLED", "Stop",]
    );
    assert_eq!(group_labels(&desktop, "Load Preset"), vec!["(none)"]);
    assert_eq!(
        group_labels(&desktop, "Notes"),
        vec!["Note Length", "Vel Scale", "Vel Curve"]
    );
    assert_eq!(
        group_labels(&desktop, "MIDI"),
        vec!["Enabled", "MIDI Out", "MIDI In", "Sync / Clock"]
    );
    assert_eq!(group_labels(&desktop, "Audio"), vec!["Polyphony", "Engine"]);
    assert_eq!(
        group_labels(&desktop, "Audio > Engine"),
        vec!["CPU Warn %", "Bus Idle", "Buf Frames"]
    );
    assert_eq!(
        group_labels(&desktop, "Setup"),
        vec![
            "Configure WiFi",
            "Backup / Restore",
            "Updates",
            "Hardware Test",
        ]
    );
    assert_eq!(
        group_labels(&desktop, "Reset"),
        vec!["Load Empty", "Load Factory"]
    );

    let mut board = config();
    board.jack_audio_required = true;
    board.audio_optimization_capacity_available = true;
    assert_eq!(
        system_labels(&board),
        vec![
            "Save Current",
            "Load Preset",
            "Master Vol",
            "Panic",
            "Sys. Info",
            "Basic Help",
            "Recording",
            "Notes",
            "MIDI",
            "Audio",
            "SD Card 2",
            "UI",
            "HDMI Video",
            "Saves",
            "Setup",
            "Reset",
            "Reboot",
            "Shutdown",
        ]
    );
    assert_eq!(
        group_labels(&board, "Audio"),
        vec![
            "USB Audio",
            "HDMI Audio",
            "Perf. Mode",
            "Polyphony",
            "Engine",
        ]
    );
    assert_eq!(
        group_labels(&board, "MIDI"),
        vec!["Enabled", "USB MIDI", "MIDI Out", "MIDI In", "Sync / Clock"]
    );
    assert_eq!(
        group_labels(&board, "SD Card 2"),
        vec!["Start Transfer", "Stop Transfer"]
    );
    assert_eq!(
        group_labels(&board, "HDMI Video"),
        vec!["Mode", "Grid Lines"]
    );

    let menu = NativeMenuModel::new(board);
    assert!(matches!(
        menu.item_for_key("preset.load").map(|item| item.value),
        Some(NativeMenuValue::Group)
    ));
    assert!(matches!(
        menu.item_for_key("preset.load.none").map(|item| item.value),
        Some(NativeMenuValue::Action(NativeMenuAction::PlatformEffect(effect)))
            if effect == "preset.refresh"
    ));
    for key in [
        "preset.saveCurrent",
        "midi.panic",
        "system.info",
        "system.controlsHelp",
        "system.reboot",
        "system.shutdown",
    ] {
        assert!(
            matches!(
                menu.item_for_key(key).map(|item| item.value),
                Some(NativeMenuValue::Action(NativeMenuAction::PlatformEffect(_)))
            ),
            "{key} should be a direct action"
        );
    }

    let mut named = config();
    named.preset_names = vec!["Alpha".into(), "Beta".into()];
    assert_eq!(group_labels(&named, "Load Preset"), vec!["Alpha", "Beta"]);
    let named_menu = NativeMenuModel::new(named);
    assert!(matches!(
        named_menu
            .item_for_key("preset.load.Alpha")
            .map(|item| item.value),
        Some(NativeMenuValue::Action(NativeMenuAction::PlatformEffect(effect)))
            if effect == "preset.load:Alpha"
    ));
}

#[test]
pub(crate) fn audio_rows_follow_explicit_jack_policy_without_shifting_keys() {
    let desktop = config();
    assert_eq!(group_labels(&desktop, "Audio"), vec!["Polyphony", "Engine"]);
    assert!(!system_labels(&desktop).iter().any(|label| label == "USB"));
    assert!(!system_labels(&desktop)
        .iter()
        .any(|label| label == "HDMI Video"));
    assert!(!NativeMenuModel::new(desktop)
        .help_targets()
        .iter()
        .any(|target| {
            target.key == "key:audioOutputs.dac"
                || target.key == "key:audioOutputs.usb"
                || target.key == "key:audioOutputs.hdmi"
                || target.key == "key:usb.midiOutEnabled"
                || target.key == "key:hdmi.mode"
        }));

    let mut pi = config();
    pi.jack_audio_required = true;
    pi.audio_optimization_capacity_available = true;
    assert_eq!(
        group_labels(&pi, "Audio"),
        vec![
            "USB Audio",
            "HDMI Audio",
            "Perf. Mode",
            "Polyphony",
            "Engine",
        ]
    );
    assert_eq!(
        group_labels(&pi, "MIDI"),
        vec!["Enabled", "USB MIDI", "MIDI Out", "MIDI In", "Sync / Clock"]
    );
    assert_eq!(
        group_labels(&pi, "SD Card 2"),
        vec!["Start Transfer", "Stop Transfer"]
    );

    let mut menu = NativeMenuModel::new(pi);
    assert!(!menu.focus_item_key("audioOutputs.dac"));
    assert!(menu.focus_item_key("audioOutputs.usb"));
    assert_eq!(menu.current_label(), Some("USB Audio"));
}

fn system_labels(config: &NativeMenuConfig) -> Vec<String> {
    let root = build_root(config.clone());
    root.children
        .iter()
        .find(|item| item.label == "System")
        .map(|system| {
            system
                .children
                .iter()
                .map(|item| item.label.clone())
                .collect()
        })
        .expect("System group")
}

fn group_labels(config: &NativeMenuConfig, path: &str) -> Vec<String> {
    let mut item = build_root(config.clone())
        .children
        .into_iter()
        .find(|child| child.label == "System")
        .expect("System group");
    for label in path.split(" > ") {
        item = item
            .children
            .into_iter()
            .find(|child| child.label == label)
            .unwrap_or_else(|| panic!("missing menu group {path}"));
    }
    item.children.into_iter().map(|child| child.label).collect()
}

#[test]
pub(crate) fn system_controls_row_is_help_action() {
    let mut menu = NativeMenuModel::new(config());
    for _ in 0..5 {
        menu.turn(1);
    }
    let _ = menu.press();
    assert!(menu.focus_item_key("system.controlsHelp"));

    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "/System");
    assert!(snapshot.lines.iter().any(|line| line == ">!Basic Help"));
    assert!(matches!(
        snapshot.selected_action,
        Some(NativeMenuAction::PlatformEffect(ref action)) if action == "system.controlsHelp"
    ));
}

#[test]
pub(crate) fn system_destructive_actions_are_grouped_and_ordered() {
    let mut menu = NativeMenuModel::new(config());

    assert!(menu.focus_item_key("system.clearAll"));
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "/SYS/Reset");
    assert!(snapshot.lines.iter().any(|line| line == ">!Load Empty"));

    assert!(menu.focus_item_key("system.reboot"));
    let snapshot = menu.snapshot();
    assert_eq!(snapshot.path, "/System");
    assert!(snapshot.lines.iter().any(|line| line == ">!Reboot"));
    let reboot_row = snapshot
        .lines
        .iter()
        .position(|line| line.contains("Reboot"))
        .expect("reboot row");
    let shutdown_row = snapshot
        .lines
        .iter()
        .position(|line| line.contains("Shutdown"))
        .expect("shutdown row");
    assert!(reboot_row < shutdown_row);

    assert!(menu.focus_item_key("system.shutdown"));
    let snapshot = menu.snapshot();
    assert_eq!(
        snapshot.lines.last().map(String::as_str),
        Some(">!Shutdown")
    );
}

#[test]
pub(crate) fn static_navigation_memory_restores_allowed_system_groups() {
    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![5];
    menu.state.cursor = 7;
    let _ = menu.press();
    let snapshot = menu.snapshot();
    let selected_row = snapshot.selected_row.expect("selected row");
    assert_eq!(snapshot.lines[selected_row], "> Note Length 150ms");

    menu.turn(1);
    assert_eq!(menu.current_label(), Some("Vel Scale"));
    menu.back();
    assert_eq!(menu.current_label(), Some("Notes"));

    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("Vel Scale"));
}

#[test]
pub(crate) fn static_navigation_memory_clears_on_rebuild() {
    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![5];
    menu.state.cursor = 7;
    let _ = menu.press();
    menu.turn(1);
    menu.back();
    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("Vel Scale"));

    menu.rebuild(config());
    menu.state.stack = vec![5];
    menu.state.cursor = 7;
    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("Note Length"));
}

#[test]
pub(crate) fn static_navigation_memory_back_while_editing_stays_in_group() {
    let mut menu = NativeMenuModel::new(config());
    menu.state.stack = vec![5];
    menu.state.cursor = 7;
    let _ = menu.press();
    menu.turn(1);
    assert_eq!(menu.current_label(), Some("Vel Scale"));

    let _ = menu.press();
    assert!(menu.state.editing);
    menu.back();
    assert!(!menu.state.editing);
    assert_eq!(menu.snapshot().path, "/SYS/Notes");
    assert_eq!(menu.current_label(), Some("Vel Scale"));

    menu.back();
    assert_eq!(menu.current_label(), Some("Notes"));
    let _ = menu.press();
    assert_eq!(menu.current_label(), Some("Vel Scale"));
}
