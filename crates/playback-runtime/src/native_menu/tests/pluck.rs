use super::*;

fn pluck_menu() -> NativeMenuModel {
    let mut cfg = config();
    cfg.instrument_types[0] = "pluck".into();
    NativeMenuModel::new(cfg)
}

#[test]
fn plucked_type_and_five_rows_use_standard_oled_navigation() {
    let mut menu = pluck_menu();
    assert!(menu.focus_item_key("instruments.0.type"));
    assert!(
        matches!(&menu.current_item().value, NativeMenuValue::Enum { options, selected }
        if options == &["none", "synth", "sampler", "midi", "fm", "pluck", "drum"] && options[*selected] == "pluck")
    );
    assert!(menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line == "> Type Plucked"));
    let slot = &menu.root.children[2].children[0].children[0];
    assert_eq!(slot.label, "I1: Plucked direct");
    assert_eq!(
        slot.children
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        [
            "Type",
            "Note Mode",
            "Plucked",
            "Mixer",
            "Auto Label",
            "Name",
            "Slot Actions"
        ]
    );
    assert_eq!(
        slot.children[2]
            .children
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        ["String", "Filter", "Volume", "Amp Env", "Filter Env"]
    );
    assert!(menu.focus_current_group_label("Plucked"));
    let _ = menu.press();
    assert_eq!(menu.snapshot().lines.len(), 5);
    assert!(menu.focus_item_key("instruments.0.pluck.decayMs"));
    let snapshot = menu.snapshot();
    assert!(
        snapshot.lines.iter().any(|line| line == "> Decay 1.5s"),
        "{:?}",
        snapshot.lines
    );
    assert!(snapshot.lines.iter().any(|line| line == "  Brightness 65%"));
    assert!(snapshot.lines.iter().any(|line| line == "  Pick Pos 25%"));
    let _ = menu.press();
    menu.turn(1);
    assert_eq!(
        menu.number_for_key("instruments.0.pluck.decayMs"),
        Some(1505)
    );
    menu.back();
    assert!(!menu.state.editing);
    assert_eq!(
        menu.number_for_key("instruments.0.pluck.decayMs"),
        Some(1505)
    );
}

#[test]
fn plucked_defaults_ranges_and_saved_values_stay_in_own_block() {
    let menu = pluck_menu();
    for (field, value, min, max, step) in [
        ("decayMs", 1500, 100, 5000, 5),
        ("brightnessPct", 65, 0, 100, 1),
        ("pickPositionPct", 25, 5, 50, 1),
        ("filter.resonance", 20, 0, 255, 1),
        ("filter.envAmountPct", 0, -100, 100, 1),
        ("filter.keyTrackingPct", 0, 0, 100, 1),
        ("amp.gainPct", 80, 0, 100, 1),
        ("amp.velocitySensitivityPct", 100, 0, 100, 1),
        ("ampEnv.attackMs", 0, 0, 5000, 5),
        ("ampEnv.decayMs", 0, 0, 5000, 5),
        ("ampEnv.sustainPct", 100, 0, 100, 1),
        ("ampEnv.releaseMs", 900, 0, 10000, 5),
        ("filterEnv.attackMs", 5, 0, 5000, 5),
        ("filterEnv.decayMs", 120, 0, 5000, 5),
        ("filterEnv.sustainPct", 70, 0, 100, 1),
        ("filterEnv.releaseMs", 180, 0, 10000, 5),
    ] {
        let key = format!("instruments.0.pluck.{field}");
        let item = menu
            .item_for_key(&key)
            .unwrap_or_else(|| panic!("missing {key}"));
        assert!(
            matches!(item.value, NativeMenuValue::Number { value: current, min: low, max: high, step: increment }
            if (current, low, high, increment) == (value, min, max, step)),
            "{key}"
        );
    }
    assert!(matches!(
        menu.item_for_key("instruments.0.pluck.filter.cutoffHz")
            .unwrap()
            .value,
        NativeMenuValue::Number {
            min: 0,
            max: 255,
            step: 1,
            ..
        }
    ));
    assert_eq!(
        menu.number_for_key("instruments.0.pluck.filter.cutoffHz"),
        Some(super::super::voice_config_read::cutoff_hz_to_display(8000))
    );
    assert!(
        matches!(&menu.item_for_key("instruments.0.pluck.filter.type").unwrap().value,
        NativeMenuValue::Enum { options, selected } if options == &["lowpass", "highpass", "bandpass", "notch"] && options[*selected] == "lowpass")
    );

    let mut cfg = config();
    cfg.instrument_types[0] = "pluck".into();
    cfg.instrument_pluck_configs[0] = serde_json::json!({
        "decayMs": 2300, "brightnessPct": 37, "pickPositionPct": 12,
        "amp": { "gainPct": 24 }, "filter": { "cutoffHz": 4000, "type": "notch" }
    });
    let menu = NativeMenuModel::new(cfg);
    assert_eq!(
        menu.number_for_key("instruments.0.pluck.decayMs"),
        Some(2300)
    );
    assert_eq!(
        menu.number_for_key("instruments.0.pluck.brightnessPct"),
        Some(37)
    );
    assert_eq!(
        menu.number_for_key("instruments.0.pluck.pickPositionPct"),
        Some(12)
    );
    assert_eq!(
        menu.number_for_key("instruments.0.pluck.amp.gainPct"),
        Some(24)
    );
    assert_eq!(
        menu.number_for_key("instruments.0.pluck.filter.cutoffHz"),
        Some(super::super::voice_config_read::cutoff_hz_to_display(4000))
    );
    assert!(matches!(
        menu.item_for_key("instruments.0.pluck.filter.type")
            .unwrap()
            .value,
        NativeMenuValue::Enum { selected: 3, .. }
    ));
    assert!(menu
        .item_for_key("instruments.0.synth.amp.gainPct")
        .is_none());
    assert!(menu.item_for_key("instruments.0.fm.index").is_none());
}

#[test]
fn plucked_picker_offers_six_numeric_controls_without_structural_targets() {
    let menu = pluck_menu();
    let picker = menu.item_for_key("aux:0:turn").expect("Aux binding picker");
    let plucked = picker
        .children
        .iter()
        .find(|row| row.label == "Shape")
        .unwrap()
        .children
        .iter()
        .find(|row| row.label == "Instruments")
        .unwrap()
        .children[0]
        .children
        .iter()
        .find(|row| row.label == "Plucked")
        .unwrap();
    assert_eq!(
        plucked
            .children
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        ["String", "Filter", "Volume"]
    );
    let bindings = plucked
        .children
        .iter()
        .flat_map(|group| group.children.iter())
        .map(|item| {
            let NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. }) =
                &item.value
            else {
                panic!("expected Plucked binding action");
            };
            assert_eq!(binding.kind, "number");
            (binding.key.as_str(), binding.min, binding.max)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        bindings,
        [
            ("instruments.0.pluck.decayMs", Some(100), Some(5000)),
            ("instruments.0.pluck.brightnessPct", Some(0), Some(100)),
            ("instruments.0.pluck.pickPositionPct", Some(5), Some(50)),
            ("instruments.0.pluck.filter.cutoffHz", Some(0), Some(255)),
            ("instruments.0.pluck.filter.resonance", Some(0), Some(255)),
            ("instruments.0.pluck.amp.gainPct", Some(0), Some(100)),
        ]
    );
    for field in [
        "filter.type",
        "filter.envAmountPct",
        "filter.keyTrackingPct",
        "amp.velocitySensitivityPct",
        "ampEnv.attackMs",
        "filterEnv.releaseMs",
    ] {
        let key = format!("aux:0:turn.instruments.0.pluck.{field}");
        assert!(find_item_by_key(&picker, &key).is_none(), "{key}");
    }
    let mut cfg = config();
    cfg.instrument_types[0] = "pluck".into();
    let numeric = parameter_picker_group_numeric("Bind".into(), "xy:x".into(), None, &cfg);
    assert!(find_item_by_key(&numeric, "xy:x.instruments.0.type").is_none());
    assert!(find_item_by_key(&numeric, "xy:x.instruments.0.pluck.filter.type").is_none());
    assert!(find_item_by_key(&numeric, "xy:x.instruments.0.pluck.decayMs").is_some());
}

#[test]
fn plucked_help_resolves_for_groups_and_numeric_controls() {
    let menu = pluck_menu();
    let targets = menu
        .help_targets()
        .into_iter()
        .filter(|target| target.key.contains(".pluck.") || target.path.contains(" > Plucked"))
        .collect::<Vec<_>>();
    assert!(targets
        .iter()
        .any(|target| target.key.ends_with(".pluck.pickPositionPct")));
    assert!(targets
        .iter()
        .any(|target| target.kind == "group" && target.path.ends_with(" > Plucked")));
    for target in targets {
        let entry = crate::native_help::resolve_native_help_entry(&target)
            .unwrap_or_else(|| panic!("Plucked help missing: {target:?}"));
        assert!(
            entry.key.contains(".pluck.") || entry.path.contains("Instrument * > Plucked"),
            "{target:?} resolved to {}",
            entry.key
        );
    }
}
