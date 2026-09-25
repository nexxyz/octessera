use super::*;

fn fm_menu() -> NativeMenuModel {
    let mut cfg = config();
    cfg.instrument_types[0] = "fm".into();
    NativeMenuModel::new(cfg)
}

fn fm_item<'a>(menu: &'a NativeMenuModel, field: &str) -> &'a NativeMenuItem {
    find_item_by_key(&menu.root, &format!("instruments.0.fm.{field}"))
        .unwrap_or_else(|| panic!("FM menu missing {field}"))
}

#[test]
fn fm_type_and_six_group_rows_use_standard_navigation() {
    let mut menu = fm_menu();
    assert!(menu.focus_item_key("instruments.0.type"));
    assert!(
        matches!(&menu.current_item().value, NativeMenuValue::Enum { options, selected }
        if options == &["none", "synth", "sampler", "midi", "fm", "pluck", "drum"] && options[*selected] == "fm")
    );
    assert!(menu.snapshot().lines.iter().any(|line| line == "> Type FM"));
    let slot = &menu.root.children[2].children[0].children[0];
    assert_eq!(slot.label, "I1: FM direct");
    assert_eq!(
        slot.children
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        [
            "Type",
            "Note Mode",
            "FM",
            "Mixer",
            "Auto Label",
            "Name",
            "Slot Actions"
        ]
    );
    let fm = &slot.children[2];
    assert_eq!(
        fm.children
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        [
            "Tone",
            "Index Env",
            "Filter",
            "Volume",
            "Amp Env",
            "Filter Env"
        ]
    );
    assert_eq!(fm.children.len(), 6);
    assert_eq!(fm.children[0].children.len(), 2);
    assert_eq!(fm.children[1].children.len(), 4);
    let fm_bindings = super::super::binding_tree::binding_tree_from_menu_item(fm, "aux.turn.0")
        .expect("FM numeric binding groups");
    assert_eq!(fm_bindings.label, "FM");
    assert!(menu.focus_current_group_label("FM"));
    let _ = menu.press();
    assert_eq!(menu.snapshot().lines.len(), 6);
    assert!(menu.focus_item_key("instruments.0.fm.ratio"));
    assert!(menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line == "> Ratio 2:1"));
    let _ = menu.press();
    menu.turn(1);
    assert_eq!(
        menu.value_for_key("instruments.0.fm.ratio"),
        Some("3".into())
    );
    assert!(menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line.contains("3:1")));
}

#[test]
fn fm_picker_exposes_only_selected_continuous_controls() {
    let mut cfg = config();
    cfg.instrument_types[0] = "fm".into();
    cfg.instrument_fm_configs[0] = serde_json::json!({
        "index": 87, "amp": { "gainPct": 23 },
        "filter": { "cutoffHz": 4000, "resonance": 64, "type": "notch" },
        "ratio": "8"
    });
    let mut menu = NativeMenuModel::new(cfg);
    assert_eq!(menu.number_for_key("instruments.0.fm.index"), Some(87));
    assert_eq!(
        menu.number_for_key("instruments.0.fm.amp.gainPct"),
        Some(23)
    );
    assert_eq!(
        menu.number_for_key("instruments.0.fm.filter.cutoffHz"),
        Some(super::super::voice_config_read::cutoff_hz_to_display(4000))
    );
    assert!(menu.focus_item_key("aux:0:turn.instruments.0.fm.index"));
    assert!(menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line == ">!FM Index"));
    let picker = menu.item_for_key("aux:0:turn").expect("Aux binding picker");
    let shape = picker
        .children
        .iter()
        .find(|item| item.label == "Shape")
        .unwrap();
    let instruments = shape
        .children
        .iter()
        .find(|item| item.label == "Instruments")
        .unwrap();
    let slot = &instruments.children[0];
    let fm = slot
        .children
        .iter()
        .find(|item| item.label == "FM")
        .unwrap();
    assert_eq!(
        fm.children
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        ["Tone", "Filter", "Volume"]
    );
    let candidates = fm
        .children
        .iter()
        .flat_map(|group| group.children.iter())
        .map(|item| {
            let NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. }) =
                &item.value
            else {
                panic!("expected FM binding action");
            };
            assert_eq!(binding.kind, "number");
            assert!(item.label.starts_with("FM "));
            assert_eq!(binding.label.as_deref(), Some(item.label.as_str()));
            assert!(format!("Current: {}", item.label).chars().count() <= 18);
            (binding.key.as_str(), binding.min, binding.max, binding.step)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        candidates,
        [
            ("instruments.0.fm.index", Some(0), Some(100), Some(1)),
            (
                "instruments.0.fm.filter.cutoffHz",
                Some(0),
                Some(255),
                Some(1)
            ),
            (
                "instruments.0.fm.filter.resonance",
                Some(0),
                Some(255),
                Some(1)
            ),
            ("instruments.0.fm.amp.gainPct", Some(0), Some(100), Some(1)),
        ]
    );
    assert!(menu
        .item_for_key("aux:0:turn.instruments.0.fm.ratio")
        .is_none());
    assert!(menu
        .item_for_key("aux:0:turn.instruments.0.fm.filter.type")
        .is_none());

    let synth = NativeMenuModel::new(config());
    assert!(synth
        .item_for_key("aux:0:turn.instruments.0.synth.amp.gainPct")
        .is_some());
    assert!(synth
        .item_for_key("aux:0:turn.instruments.0.fm.index")
        .is_none());
}

#[test]
fn fm_defaults_ranges_and_saved_values_are_independent_from_synth() {
    let menu = fm_menu();
    assert!(
        matches!(&fm_item(&menu, "ratio").value, NativeMenuValue::Enum { options, selected }
        if options == &["0.5", "1", "2", "3", "4", "5", "6", "8"] && options[*selected] == "2")
    );
    for (field, expected, min, max, step) in [
        ("index", 50, 0, 100, 1),
        ("indexEnv.attackMs", 0, 0, 5000, 5),
        ("indexEnv.decayMs", 250, 0, 5000, 5),
        ("indexEnv.sustainPct", 20, 0, 100, 1),
        ("indexEnv.releaseMs", 120, 0, 10000, 5),
        ("amp.gainPct", 80, 0, 100, 1),
        ("amp.velocitySensitivityPct", 100, 0, 100, 1),
        ("ampEnv.attackMs", 5, 0, 5000, 5),
        ("ampEnv.decayMs", 300, 0, 5000, 5),
        ("ampEnv.sustainPct", 70, 0, 100, 1),
        ("ampEnv.releaseMs", 350, 0, 10000, 5),
        ("filterEnv.attackMs", 5, 0, 5000, 5),
        ("filterEnv.decayMs", 120, 0, 5000, 5),
        ("filterEnv.sustainPct", 70, 0, 100, 1),
        ("filterEnv.releaseMs", 180, 0, 10000, 5),
        ("filter.resonance", 20, 0, 255, 1),
        ("filter.envAmountPct", 0, -100, 100, 1),
        ("filter.keyTrackingPct", 0, 0, 100, 1),
    ] {
        assert!(
            matches!(fm_item(&menu, field).value, NativeMenuValue::Number { value, min: low, max: high, step: increment }
            if (value, low, high, increment) == (expected, min, max, step)),
            "{field}"
        );
    }
    assert!(matches!(
        fm_item(&menu, "filter.cutoffHz").value,
        NativeMenuValue::Number {
            min: 0,
            max: 255,
            step: 1,
            ..
        }
    ));
    assert!(
        matches!(&fm_item(&menu, "filter.type").value, NativeMenuValue::Enum { options, selected }
        if options == &["lowpass", "highpass", "bandpass", "notch"] && options[*selected] == "lowpass")
    );

    let mut cfg = config();
    cfg.instrument_types[0] = "fm".into();
    cfg.instrument_fm_configs[0] = serde_json::json!({
        "ratio": "0.5", "index": 87, "filter": { "type": "notch", "cutoffHz": 4000 },
        "amp": { "gainPct": 23 }, "indexEnv": { "decayMs": 400 }
    });
    let menu = NativeMenuModel::new(cfg);
    assert_eq!(
        menu.value_for_key("instruments.0.fm.ratio"),
        Some("0.5".into())
    );
    assert!(matches!(
        fm_item(&menu, "index").value,
        NativeMenuValue::Number { value: 87, .. }
    ));
    assert!(matches!(
        fm_item(&menu, "amp.gainPct").value,
        NativeMenuValue::Number { value: 23, .. }
    ));
    assert!(matches!(
        fm_item(&menu, "indexEnv.decayMs").value,
        NativeMenuValue::Number { value: 400, .. }
    ));
    assert!(matches!(
        fm_item(&menu, "filter.type").value,
        NativeMenuValue::Enum { selected: 3, .. }
    ));
    assert!(find_item_by_key(&menu.root, "instruments.0.synth.amp.gainPct").is_none());
}

#[test]
fn fm_enum_and_group_help_resolve_to_fm_entries() {
    let menu = fm_menu();
    let targets = menu
        .help_targets()
        .into_iter()
        .filter(|target| target.key.contains(".fm.") || target.path.contains(" > FM"))
        .collect::<Vec<_>>();
    assert!(targets
        .iter()
        .any(|target| target.key.ends_with(".fm.ratio")));
    assert!(targets
        .iter()
        .any(|target| target.kind == "group" && target.path.ends_with(" > FM")));
    for target in targets {
        let entry = crate::native_help::resolve_native_help_entry(&target)
            .unwrap_or_else(|| panic!("FM help missing: {target:?}"));
        assert!(
            entry.key.contains(".fm.") || entry.path.contains("Instrument * > FM"),
            "{target:?} resolved to {}",
            entry.key
        );
    }
}
