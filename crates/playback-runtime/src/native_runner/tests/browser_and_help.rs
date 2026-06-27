use super::*;

#[test]
fn sample_browser_opens_lists_and_picks_sample() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::SampleListRequest {
                instrument_slot: 0,
                sample_slot: 0,
                dir: "".into(),
            }]
    )));

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SampleListResult {
                instrument_slot: 0,
                sample_slot: 0,
                dir: "".into(),
                entries: vec![SampleEntry {
                    name: "kick.wav".into(),
                    path: "Drums/kick.wav".into(),
                    is_dir: false,
                }],
            },
        })
        .unwrap();
    runner.menu.state.stack = vec![2, 0, 0, 2, 1];
    runner.menu.state.cursor = 1;

    let preview = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert!(preview.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::SamplePreview {
                    instrument_slot: 0,
                    sample_slot: 0,
                    path: "Drums/kick.wav".into(),
                    velocity: 100,
                },
            }]
    )));

    runner.menu.state.stack = vec![2, 0, 0, 2, 1];
    runner.menu.state.cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    let snapshot = runner.snapshot().unwrap();

    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["slots"][0]["path"],
        "Drums/kick.wav"
    );
}

#[test]
fn sample_browser_shows_favourite_toggle_and_updates_runtime_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.sample_browser = Some(NativeSampleBrowser {
        instrument_slot: 0,
        sample_slot: 0,
        dir: "Samples".into(),
        entries: vec![SampleEntry {
            name: "kick.wav".into(),
            path: "Samples/kick.wav".into(),
            is_dir: false,
        }],
    });
    runner.menu.state.stack = vec![2, 0, 0, 2, 1];
    runner.menu.state.cursor = 3;
    runner.menu.rebuild(runner.menu_config());

    let snapshot = runner.menu.snapshot();
    assert_eq!(
        snapshot.lines,
        vec!["  !..", "  !kick.wav", "", "> !Set favourite"]
    );

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.toast.as_ref().unwrap().message, "Favourite set");
    assert_eq!(runner.sample_favourite_dirs, vec![String::from("Samples")]);

    let snapshot = runner.menu.snapshot();
    assert_eq!(snapshot.lines[3], "> !Remove favourite");

    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["sampleFavouriteDirs"],
        json!(["Samples"])
    );

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.sample_browser = runner.sample_browser.clone();
    loaded.apply_config_payload(payload).unwrap();
    assert_eq!(loaded.sample_favourite_dirs, vec![String::from("Samples")]);

    loaded.menu.state.stack = vec![2, 0, 0, 2, 1];
    loaded.menu.state.cursor = 3;
    assert_eq!(loaded.menu.snapshot().lines[3], "> !Remove favourite");

    let _ = loaded
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(loaded.toast.as_ref().unwrap().message, "Favourite removed");
    assert!(loaded.sample_favourite_dirs.is_empty());
}

#[test]
fn sample_browser_shows_non_deletable_builtin_favourites() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        sample_builtin_favourite_dirs: vec![String::new(), "sd-card".into()],
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.sample_browser = Some(NativeSampleBrowser {
        instrument_slot: 0,
        sample_slot: 0,
        dir: String::new(),
        entries: vec![],
    });
    runner.menu.state.stack = vec![2, 0, 0, 2, 1];
    runner.menu.rebuild(runner.menu_config());

    let snapshot = runner.menu.snapshot();
    assert!(snapshot.lines.contains(&"  ![★ Samples]".into()));
    assert!(snapshot.lines.contains(&"  ![★ SD card]".into()));

    runner.sample_browser = Some(NativeSampleBrowser {
        instrument_slot: 0,
        sample_slot: 0,
        dir: "sd-card".into(),
        entries: vec![],
    });
    runner.menu.rebuild(runner.menu_config());
    let snapshot = runner.menu.snapshot();
    assert!(snapshot
        .lines
        .iter()
        .any(|line| line.contains("Built-in favourite")));
    assert!(!snapshot
        .lines
        .iter()
        .any(|line| line.contains("Remove favourite")));
}

#[test]
fn sample_browser_error_surfaces_host_message_and_empty_browser() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.startup_splash_presented = true;
    runner.toast = None;
    runner.sample_browser = Some(NativeSampleBrowser {
        instrument_slot: 0,
        sample_slot: 0,
        dir: "sd-card".into(),
        entries: vec![],
    });
    let host_message = "SD card missing";

    runner
        .apply_store_result(RuntimeStoreResult::SampleListError {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "sd-card".into(),
            message: host_message.into(),
        })
        .unwrap();

    let browser = runner.sample_browser.as_ref().unwrap();
    assert_eq!(browser.instrument_slot, 0);
    assert_eq!(browser.sample_slot, 0);
    assert_eq!(browser.dir, "sd-card");
    assert!(browser.entries.is_empty());
    assert_eq!(runner.toast.as_ref().unwrap().message, host_message);
    assert_eq!(runner.snapshot().unwrap()["display"]["toast"], host_message);
}

#[test]
fn fn_shift_enter_opens_contextual_help_and_enter_closes_it() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![2];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_shift", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let modifiers = snapshot_from(
        &runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "button_combined_modifier", "pressed": true }),
            })
            .unwrap(),
    );
    assert_eq!(modifiers["settings"]["shiftHeld"], false);
    assert_eq!(modifiers["settings"]["fnHeld"], false);
    assert_eq!(modifiers["settings"]["combinedModifierHeld"], true);
    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let snapshot = snapshot_from(&opened);
    assert_eq!(snapshot["display"]["title"], "Help: Instruments");
    let help_lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(help_lines
        .iter()
        .any(|line| line.as_str().unwrap_or("").contains("destination")));
    assert_eq!(help_lines.last().unwrap(), "> Close");

    let closed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&closed)["display"]["title"], "L3: Voice");
}

#[test]
fn submenu_snapshot_does_not_append_transport_status_line() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![1];
    let messages = runner.messages_with_snapshot().unwrap();
    let lines = snapshot_from(&messages)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert!(lines
        .iter()
        .all(|line| !line.contains("Stopped") && !line.contains("BPM")));
}

#[test]
fn instrument_pan_menu_edit_moves_monotonically_from_current_value() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].pan_pos = 10;
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 3];
    runner.menu.state.cursor = 2;
    runner.menu.state.editing = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.instruments[0].pan_pos, 11);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.instruments[0].pan_pos, 12);
}

#[test]
fn active_part_trigger_toggle_suppresses_and_restores_with_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.ui.fn_held = true;
    let off = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "zero");
    assert_eq!(snapshot_from(&off)["display"]["toast"], "P1 triggers off");

    let on = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "full");
    assert_eq!(snapshot_from(&on)["display"]["toast"], "P1 triggers full");
}

#[test]
fn repeated_autosaves_increment_flash_serial() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    let first = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    let first_serial = snapshot_from(&first)["settings"]["autoSaveFlashSerial"]
        .as_u64()
        .unwrap();

    let second = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 0 }),
        })
        .unwrap();
    let second_serial = snapshot_from(&second)["settings"]["autoSaveFlashSerial"]
        .as_u64()
        .unwrap();
    assert!(second_serial > first_serial);
}

#[test]
fn config_load_queues_midi_port_selection_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "midi": {
                    "enabled": true,
                    "outId": "out1",
                    "inId": "in1"
                }
            }
        }))
        .unwrap();

    let messages = runner.messages_with_snapshot().unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![
                RuntimePlatformEffect::MidiSelectOutput { id: Some("out1".into()) },
                RuntimePlatformEffect::MidiSelectInput { id: Some("in1".into()) },
            ]
    )));
}

#[test]
fn contextual_help_includes_midi_output_guidance() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 4];
    runner.menu.state.cursor = 2;
    runner.ui.combined_modifier_held = true;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let lines = snapshot_from(&opened)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(lines.contains("output"));
}

#[test]
fn contextual_help_resolves_life_params_and_scrolls_to_bottom() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        behavior_config: json!({ "randomCellsPerTick": 12, "randomTickInterval": 1 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.menu.state.stack = vec![0, 0];
    runner.menu.state.cursor = 4;
    runner.ui.combined_modifier_held = true;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    let lines = snapshot_from(&opened)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(lines.contains("random cells"));
    runner.ui.combined_modifier_held = false;

    runner.turn_help_popup(2);
    let help = runner.help_popup.as_ref().unwrap();
    assert_eq!(
        help.scroll,
        help.lines.len().saturating_sub(OLED_BODY_ROWS - 1)
    );
}

#[test]
fn contextual_help_scrolls_and_back_closes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.cursor = 5;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_shift", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    if let Some(help) = &mut runner.help_popup {
        help.lines = vec!["l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8"]
            .into_iter()
            .map(String::from)
            .collect();
    }

    let scrolled = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.help_popup.as_ref().unwrap().scroll, 2);
    assert!(snapshot_from(&scrolled)["display"]["lines"]
        .as_array()
        .unwrap()[0]
        .as_str()
        .unwrap()
        .contains("l3"));

    let closed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();
    assert!(runner.help_popup.is_none());
    assert_eq!(snapshot_from(&closed)["display"]["title"], "MENU");
}
