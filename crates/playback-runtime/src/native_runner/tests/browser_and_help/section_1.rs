use super::*;

#[test]
pub(crate) fn sample_browser_opens_lists_and_picks_sample() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![2, 0, 0, 2];
    runner.menu.state.cursor = 2;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
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
    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
    runner.menu.state.cursor = 1;

    let preview = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
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

    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
    runner.menu.state.cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetInstrumentSlot { instrument_slot: 0, .. }
            ))
    )));
    let snapshot = runner.snapshot().unwrap();

    assert_eq!(
        snapshot["settings"]["instruments"][0]["sample"]["slots"][0]["path"],
        "Drums/kick.wav"
    );
    assert!(runner.sample_browser.is_none());
    assert_eq!(snapshot["display"]["title"], "S/Instruments/I1: samp direc");
    assert_eq!(snapshot["display"]["lines"][2], "> S1 Browse >");
    runner.make_deferred_menu_apply_due_for_test();
    let autosave = runner.flush_deferred_menu_apply().unwrap();
    assert!(autosave.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { mode, .. }
                    if mode.as_deref() == Some("deferred")
            ))
    )));
}

#[test]
pub(crate) fn sample_browser_shows_favourite_toggle_and_updates_runtime_config() {
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
    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
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
            request_snapshot: None,
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

    loaded.menu.state.stack = vec![2, 0, 0, 2, 2];
    loaded.menu.state.cursor = 3;
    assert_eq!(loaded.menu.snapshot().lines[3], "> !Remove favourite");

    let _ = loaded
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(loaded.toast.as_ref().unwrap().message, "Favourite removed");
    assert!(loaded.sample_favourite_dirs.is_empty());
}

#[test]
pub(crate) fn sample_browser_shows_non_deletable_builtin_favourites() {
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
    runner.menu.state.stack = vec![2, 0, 0, 2, 2];
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
pub(crate) fn sample_browser_error_surfaces_host_message_and_empty_browser() {
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
