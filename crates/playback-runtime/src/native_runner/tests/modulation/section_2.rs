use super::*;

#[test]
pub(crate) fn voice_stealing_param_mod_emits_full_audio_config_command() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "sound.voiceStealingMode".into(),
        label: Some("Steal".into()),
        kind: "enum".into(),
        min: None,
        max: None,
        step: None,
        options: vec!["auto-balanced".into(), "auto-hard".into()],
        invert: false,
    });
    let intents = vec![CellTriggerIntent {
        x: 7,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    }];

    runner.apply_runtime_modulation(&intents, 0);
    let messages = runner.messages_with_snapshot().unwrap();

    let config = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.iter().find_map(|command| {
                if let RuntimeAudioCommand::SetAudioConfig { config, .. } = command {
                    Some(config)
                } else {
                    None
                }
            }),
            _ => None,
        })
        .expect("full audio config command");
    assert_eq!(config["voiceStealingMode"], "auto-hard");
}

#[test]
pub(crate) fn shift_grid_param_mod_mapping_cycles_slots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    };

    assert!(runner.apply_param_mod_mapping(0, 0, binding.clone()));
    assert_eq!(runner.param_mods[0].x[0].as_ref().unwrap().key, binding.key);
    assert_eq!(runner.param_mods[0].y[0].as_ref().unwrap().key, binding.key);
    assert!(!runner.param_mods[0].x[0].as_ref().unwrap().invert);
    assert!(runner.config_dirty);

    assert!(runner.apply_param_mod_mapping(0, 0, binding.clone()));
    assert!(runner.param_mods[0].x[0].as_ref().unwrap().invert);
    assert!(runner.param_mods[0].y[0].as_ref().unwrap().invert);

    assert!(runner.apply_param_mod_mapping(0, 0, binding.clone()));
    assert!(runner.param_mods[0].x[0].is_none());
    assert!(runner.param_mods[0].y[0].is_none());

    assert!(runner.apply_param_mod_mapping(2, 1, binding.clone()));
    assert_eq!(runner.param_mods[0].x[1].as_ref().unwrap().key, binding.key);
    assert!(runner.param_mods[0].y[1].is_none());

    assert!(runner.apply_param_mod_mapping(1, 4, binding.clone()));
    assert_eq!(runner.param_mods[0].y[1].as_ref().unwrap().key, binding.key);

    assert!(runner.apply_param_mod_mapping(1, 1, binding));
    assert!(runner.param_mods[0].x[1].as_ref().unwrap().invert);
    assert!(runner.param_mods[0].y[1].as_ref().unwrap().invert);
}

#[test]
pub(crate) fn shift_grid_param_mod_overlay_marks_lanes_and_combined_cells() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        options: vec![],
        invert: false,
    };
    let mut inverted = binding.clone();
    inverted.invert = true;
    runner.param_mods[0].x[0] = Some(binding);
    runner.param_mods[0].y[1] = Some(inverted);

    runner.menu.turn(2);
    runner.menu.press();
    runner.menu.press();
    runner.menu.press();
    runner.menu.turn(3);
    runner.menu.press();
    runner.menu.turn(1);
    runner.ui.shift_held = true;

    let snapshot = runner.snapshot().unwrap();
    let cells = led_cells(&snapshot);

    assert_eq!(
        cells[display_index(3, 0)],
        json!({ "r": 0, "g": 255, "b": 120 })
    );
    assert_eq!(
        cells[display_index(1, 3)],
        json!({ "r": 255, "g": 0, "b": 90 })
    );
    assert_eq!(
        cells[display_index(3, 1)],
        json!({ "r": 18, "g": 18, "b": 24 })
    );
    assert_eq!(
        cells[display_index(0, 0)],
        json!({ "r": 255, "g": 255, "b": 255 })
    );
    assert_eq!(
        cells[display_index(1, 1)],
        json!({ "r": 255, "g": 255, "b": 255 })
    );
}
