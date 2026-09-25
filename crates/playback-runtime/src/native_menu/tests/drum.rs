use super::*;

fn drum_menu() -> NativeMenuModel {
    let mut cfg = config();
    cfg.instrument_types[0] = "drum".into();
    NativeMenuModel::new(cfg)
}

#[test]
fn drum_type_hides_note_mode_and_keeps_seven_rows() {
    let mut menu = drum_menu();
    assert!(menu.focus_item_key("instruments.0.type"));
    assert!(
        matches!(&menu.current_item().value, NativeMenuValue::Enum { options, selected }
        if options == &["none", "synth", "sampler", "midi", "fm", "pluck", "drum"] && options[*selected] == "drum")
    );
    assert!(menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line == "> Type Drum"));
    let slot = &menu.root.children[2].children[0].children[0];
    assert_eq!(slot.label, "I1: Drum direct");
    assert_eq!(
        slot.children
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        [
            "Type",
            "Drum",
            "Mixer",
            "Auto Label",
            "Name",
            "Slot Actions"
        ]
    );
    let drum = &slot.children[1];
    assert_eq!(
        drum.children
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        [
            "Voice",
            "Edit",
            "Assign",
            "Cell Tune",
            "Preview",
            "Filter",
            "Volume"
        ]
    );
    assert_eq!(
        drum.children[1]
            .children
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        ["Sound", "Tune", "Decay", "Tone", "Attack"]
    );
    assert!(menu.item_for_key("instruments.0.noteBehavior").is_none());
    assert!(menu
        .item_for_key("instruments.0.drum.ampEnv.attackMs")
        .is_none());
    assert!(menu
        .item_for_key("instruments.0.drum.filterEnv.attackMs")
        .is_none());
    assert!(menu.focus_current_group_label("Drum"));
    let _ = menu.press();
    assert_eq!(menu.snapshot().lines.len(), 7);
    assert!(menu.focus_item_key("instruments.0.drum.voices.0.tonePct"));
    let _ = menu.press();
    menu.turn(1);
    assert_eq!(
        menu.number_for_key("instruments.0.drum.voices.0.tonePct"),
        Some(36)
    );
    menu.back();
    assert!(!menu.state.editing);
}

#[test]
fn drum_voice_labels_follow_current_sound_and_style_defaults() {
    let menu = drum_menu();
    let voice = menu.item_for_key("instruments.0.drum.voice").unwrap();
    assert!(
        matches!(&voice.value, NativeMenuValue::Enum { options, selected }
        if options == &["V1: Kick", "V2: Snare", "V3: Closed Hat", "V4: Open Hat", "V5: Low Tom", "V6: High Tom", "V7: Clap", "V8: Rim"] && *selected == 0)
    );
    for (index, decay, tone) in [
        (0, 420, 35),
        (1, 220, 70),
        (2, 85, 90),
        (3, 650, 85),
        (4, 500, 50),
        (5, 320, 60),
        (6, 240, 80),
        (7, 95, 80),
    ] {
        let mut cfg = config();
        cfg.instrument_types[0] = "drum".into();
        cfg.instrument_drum_selected_voices[0] = index;
        let menu = NativeMenuModel::new(cfg);
        for (field, value, min, max, step) in [
            ("tuneSemis", 0, -12, 12, 1),
            ("decayMs", decay, 20, 2000, 5),
            ("tonePct", tone, 0, 100, 1),
            ("attackMs", 0, 0, 50, 1),
        ] {
            let key = format!("instruments.0.drum.voices.{index}.{field}");
            assert!(
                matches!(menu.item_for_key(&key).unwrap().value,
                NativeMenuValue::Number { value: current, min: low, max: high, step: increment }
                if (current, low, high, increment) == (value, min, max, step)),
                "{key}"
            );
        }
    }

    let mut cfg = config();
    cfg.instrument_types[0] = "drum".into();
    cfg.instrument_drum_configs[0] = serde_json::json!({"voices": [{
        "sound": "snare", "tuneSemis": -3, "decayMs": 314, "tonePct": 44, "attackMs": 12
    }]});
    let mut menu = NativeMenuModel::new(cfg.clone());
    assert!(menu.focus_item_key("instruments.0.drum.voices.0.sound"));
    assert!(menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line == "> Sound Snare"));
    let label = menu.item_for_key("instruments.0.drum.voice").unwrap();
    assert!(
        matches!(&label.value, NativeMenuValue::Enum { options, .. } if options[0] == "V1: Snare")
    );
    assert_eq!(
        menu.number_for_key("instruments.0.drum.voices.0.decayMs"),
        Some(314)
    );
    cfg.instrument_drum_configs[0]["voices"][0] = serde_json::json!({"sound": "clap"});
    menu.rebuild(cfg);
    assert!(menu.focus_item_key("instruments.0.drum.voices.0.sound"));
    assert!(menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line == "> Sound Clap"));
    let label = menu.item_for_key("instruments.0.drum.voice").unwrap();
    assert!(
        matches!(&label.value, NativeMenuValue::Enum { options, .. } if options[0] == "V1: Clap")
    );
    assert_eq!(
        menu.number_for_key("instruments.0.drum.voices.0.decayMs"),
        Some(240)
    );
    assert_eq!(
        menu.number_for_key("instruments.0.drum.voices.0.tonePct"),
        Some(80)
    );
    assert!(menu
        .item_for_key("instruments.0.synth.amp.gainPct")
        .is_none());
}

#[test]
fn drum_slot_filter_volume_actions_and_binding_picker_keep_voice_scope() {
    let menu = drum_menu();
    assert!(matches!(
        menu.item_for_key("instruments.0.drum.filter.cutoffHz")
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
        menu.number_for_key("instruments.0.drum.amp.gainPct"),
        Some(80)
    );
    assert_eq!(
        menu.number_for_key("instruments.0.drum.amp.velocitySensitivityPct"),
        Some(100)
    );
    assert!(
        matches!(&menu.item_for_key("instruments.0.drum.filter.type").unwrap().value,
        NativeMenuValue::Enum { options, selected } if options == &["lowpass", "highpass", "bandpass", "notch"] && options[*selected] == "lowpass")
    );
    for (key, effect) in [
        ("drum.assign.0.0", "drum.assign:0:0"),
        ("drum.cellTune.0", "drum.cellTune:0"),
        ("drum.preview.0.0", "drum.preview:0:0"),
    ] {
        assert!(matches!(menu.item_for_key(key).unwrap().value,
            NativeMenuValue::Action(NativeMenuAction::PlatformEffect(actual)) if actual == effect));
    }
    let picker = menu.item_for_key("aux:0:turn").expect("Aux picker");
    let drum = picker
        .children
        .iter()
        .find(|item| item.label == "Shape")
        .unwrap()
        .children
        .iter()
        .find(|item| item.label == "Instruments")
        .unwrap()
        .children[0]
        .children
        .iter()
        .find(|item| item.label == "Drum")
        .unwrap();
    assert_eq!(
        drum.children
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        ["Edit", "Filter", "Volume"]
    );
    let bindings = drum
        .children
        .iter()
        .flat_map(|group| group.children.iter())
        .map(|item| {
            let NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. }) =
                &item.value
            else {
                panic!("expected Drum binding action");
            };
            assert_eq!(binding.kind, "number");
            (binding.key.as_str(), binding.label.as_deref().unwrap_or(""))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        bindings,
        [
            ("instruments.0.drum.voices.0.tuneSemis", "V1 Tune"),
            ("instruments.0.drum.voices.0.decayMs", "V1 Decay"),
            ("instruments.0.drum.voices.0.tonePct", "V1 Tone"),
            ("instruments.0.drum.voices.0.attackMs", "V1 Attack"),
            ("instruments.0.drum.filter.cutoffHz", "Cutoff"),
            ("instruments.0.drum.filter.resonance", "Res"),
            ("instruments.0.drum.amp.gainPct", "Gain"),
        ]
    );
    for key in [
        "instruments.0.drum.voice",
        "instruments.0.drum.voices.0.sound",
        "instruments.0.drum.filter.type",
        "instruments.0.drum.amp.velocitySensitivityPct",
    ] {
        assert!(
            find_item_by_key(&picker, &format!("aux:0:turn.{key}")).is_none(),
            "{key}"
        );
    }
    let mut cfg = config();
    cfg.instrument_types[0] = "drum".into();
    let numeric = parameter_picker_group_numeric("Bind".into(), "xy:x".into(), None, &cfg);
    assert!(find_item_by_key(&numeric, "xy:x.instruments.0.type").is_none());
    assert!(find_item_by_key(&numeric, "xy:x.instruments.0.drum.voice").is_none());
    assert!(find_item_by_key(&numeric, "xy:x.instruments.0.drum.voices.0.sound").is_none());
    assert!(find_item_by_key(&numeric, "xy:x.instruments.0.drum.voices.0.decayMs").is_some());
    cfg.instrument_drum_selected_voices[0] = 5;
    let menu = NativeMenuModel::new(cfg);
    let picker = menu.item_for_key("aux:0:turn").unwrap();
    let key = "aux:0:turn.instruments.0.drum.voices.5.decayMs";
    let item = find_item_by_key(&picker, key).unwrap();
    assert!(matches!(&item.value,
        NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. })
        if binding.label.as_deref() == Some("V6 Decay")));
    assert!(find_item_by_key(&picker, "aux:0:turn.instruments.0.drum.voices.0.decayMs").is_none());
}

#[test]
fn drum_help_covers_voice_edits_actions_and_play_slot() {
    let menu = drum_menu();
    let targets = menu
        .help_targets()
        .into_iter()
        .filter(|target| {
            target.key.contains(".drum.")
                || target.key.starts_with("action:drum.")
                || target.path.contains(" > Drum")
                || target.key == "key:play.page.drums"
                || target.key == "key:play.drums.slot"
        })
        .collect::<Vec<_>>();
    assert!(targets
        .iter()
        .any(|target| target.key == "key:instruments.*.drum.voice"));
    assert!(targets
        .iter()
        .any(|target| target.key == "key:play.drums.slot"));
    for target in targets {
        let entry = crate::native_help::resolve_native_help_entry(&target)
            .unwrap_or_else(|| panic!("Drum help missing: {target:?}"));
        assert!(
            entry.key.contains(".drum.")
                || entry.key.starts_with("action:drum.")
                || entry.path.contains("Instrument * > Drum")
                || entry.key == "key:play.page.drums"
                || entry.key == "key:play.drums.slot",
            "{target:?} resolved to {}",
            entry.key
        );
    }
}
