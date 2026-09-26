use super::*;

fn converted_scene(runner: &NativeRunner, audio: bool) -> Value {
    let scene = runner.capture_presentation_scene(audio).unwrap();
    fn assert_send<T: Send>() {}
    assert_send::<PresentationScene>();
    std::thread::spawn(move || scene.into_snapshot())
        .join()
        .unwrap()
}

#[test]
fn shipped_default_scene_preserves_native_menu_and_transient_contract() {
    let payload: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let start = Instant::now();
    runner.test_set_display_time(start);
    let initial = converted_scene(&runner, true);
    assert_eq!(initial["display"]["title"], "MENU");
    assert_eq!(
        initial["display"]["lines"],
        json!([
            "> Build >",
            "  Link >",
            "  Shape >",
            "  Play >",
            "",
            "  System >"
        ])
    );
    assert_eq!(initial["selectedRow"], 0);
    assert_eq!(initial["settings"]["displayBrightness"], 75);
    assert_eq!(
        initial["settings"]["instruments"].as_array().unwrap().len(),
        8
    );
    assert_eq!(initial["settings"]["panPositions"], PAN_POSITION_COUNT);
    for pressed in [true, false] {
        runner
            .send(HostMessage::DeviceInput {
                input: json!({"type": "button_s", "pressed": pressed}),
                request_snapshot: None,
            })
            .unwrap();
    }
    runner.test_set_display_time(start + Duration::from_millis(91));
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let beat = converted_scene(&runner, false);
    assert_eq!(beat["transport"]["playing"], true);
    assert_eq!(beat["transport"]["ppqnPulse"], 24);
    assert_eq!(beat["transportFlash"], "beat");
    assert_eq!(beat["eventDotOn"], true);
    assert!(beat["settings"].get("instruments").is_none());
    runner.test_set_display_time(start + Duration::from_millis(300));
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let expired = converted_scene(&runner, true);
    assert_eq!(expired["transportFlash"], "none");
    assert_eq!(expired["eventDotOn"], false);
    assert_eq!(expired["display"]["title"], "MENU");
    runner
        .send(HostMessage::DeviceInput {
            input: json!({"type": "encoder_press", "id": "main"}),
            request_snapshot: None,
        })
        .unwrap();
    let build = converted_scene(&runner, false);
    assert_eq!(build["display"]["title"], "/Build");
    assert_eq!(build["display"]["lines"][0], "> L1: life >");
    assert_eq!(build["selectedRow"], 0);

    assert!(runner.menu.focus_item_key("system.controlsHelp"));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({"type": "encoder_press", "id": "main"}),
            request_snapshot: None,
        })
        .unwrap();
    let help = converted_scene(&runner, false);
    assert_eq!(help["display"]["title"], "Help: Basic Help");
    assert_eq!(
        help["display"]["lines"].as_array().unwrap().last().unwrap(),
        "> Close"
    );
}

#[test]
fn audio_bearing_scene_captures_fast_aux_value_without_revision_change() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let payload: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    for pressed in [true, false] {
        runner
            .send(HostMessage::DeviceInput {
                input: json!({"type": "button_s", "pressed": pressed}),
                request_snapshot: None,
            })
            .unwrap();
    }
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.synth.osc1.levelPct".into()),
        press_action: None,
    });
    let before = runner.instruments[0].synth_config["osc1"]["levelPct"].clone();
    assert_eq!(before, 80);
    let revision = runner.audio_config_revision;
    for _ in 0..2 {
        let messages = runner
            .send(HostMessage::DeviceInput {
                input: json!({"type": "encoder_turn", "id": "aux1", "delta": 1}),
                request_snapshot: None,
            })
            .unwrap();
        let audio = messages
            .iter()
            .position(|message| matches!(message, RunnerMessage::AudioCommands { .. }))
            .unwrap();
        let snapshot = messages
            .iter()
            .position(|message| matches!(message, RunnerMessage::Snapshot { .. }))
            .unwrap();
        assert!(audio < snapshot);
    }
    assert_eq!(runner.audio_config_revision, revision);
    let scene = runner.capture_presentation_scene(true).unwrap();
    let deferred = scene.into_snapshot();
    assert_eq!(
        deferred["settings"]["instruments"][0]["synth"]["osc1"]["levelPct"],
        82
    );
    assert_eq!(
        deferred["settings"]["instruments"][0]["synth"]["osc1"]["levelPct"],
        runner.instruments[0].synth_config["osc1"]["levelPct"]
    );
    assert_eq!(
        deferred["settings"]["instruments"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    assert!(deferred["settings"]["mixer"].is_object());
    assert_eq!(deferred["settings"]["audioConfigRevision"], revision);
}

#[test]
fn due_scene_preserves_audio_config_inclusion_and_queue_revision() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let due = runner
        .capture_next_presentation_scene()
        .unwrap()
        .into_snapshot();
    assert_eq!(due["settings"]["instruments"].as_array().unwrap().len(), 8);
    assert!(due["settings"]["mixer"]["master"]["slots"].is_array());
    assert_eq!(
        runner.last_snapshot_audio_config_revision,
        Some(runner.audio_config_revision)
    );
    let ordinary = runner
        .capture_next_presentation_scene()
        .unwrap()
        .into_snapshot();
    assert!(ordinary["settings"].get("instruments").is_none());
    assert!(ordinary["settings"].get("mixer").is_none());
    assert!(ordinary["settings"].get("panPositions").is_none());
    assert_eq!(
        ordinary["settings"]["audioConfigRevision"],
        due["settings"]["audioConfigRevision"]
    );
}

#[test]
fn scene_keeps_display_priority_hdmi_modes_and_sleep_snapshot_shape() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let payload: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let model = runner.engine.model().unwrap();
    assert!(model.cells.iter().any(|alive| *alive));
    assert!(model.cells.iter().any(|alive| !*alive));
    assert!(model.cells[GRID_WIDTH + 2]);
    assert!(!model.cells[0]);
    let rendered = runner
        .capture_presentation_scene(false)
        .unwrap()
        .into_snapshot();
    for (index, alive) in model.cells.iter().enumerate() {
        let x = index % GRID_WIDTH;
        let y = index / GRID_WIDTH;
        let display_index = (GRID_HEIGHT - 1 - y) * GRID_WIDTH + x;
        assert_eq!(rendered["leds"]["active"][display_index], *alive);
    }
    runner.display.help_popup = Some(NativeHelpPopup {
        title: "Help".into(),
        lines: vec!["Use the grid".into()],
        scroll: 0,
    });
    let help = converted_scene(&runner, false);
    assert_eq!(help["display"]["title"], "Help");
    assert_eq!(help["display"]["lines"], json!(["Use the grid", "> Close"]));
    assert_eq!(help["selectedRow"], 1);
    runner.display.confirm_dialog = Some(NativeConfirmDialog {
        title: "Confirm".into(),
        lines: vec!["Really?".into()],
        options: vec!["Cancel".into(), "Confirm".into()],
        cursor: 1,
        action: NativeMenuAction::NavigateBack,
        cancel_toast: None,
        confirm_before_execute: false,
    });
    let confirm = converted_scene(&runner, false);
    assert_eq!(confirm["display"]["title"], "Confirm");
    assert_eq!(
        confirm["display"]["lines"],
        json!(["Really?", "  Cancel", "> Confirm"])
    );
    assert_eq!(confirm["selectedRow"], 2);
    runner.display.runtime_error_presentation = Some(NativeRuntimeErrorPresentation {
        title: "Fault".into(),
        lines: vec!["Keep playing".into()],
    });
    let error = converted_scene(&runner, false);
    assert_eq!(error["display"]["title"], "Fault");
    assert_eq!(error["display"]["lines"], json!(["Keep playing"]));
    assert!(error["selectedRow"].is_null());
    runner.display.runtime_error_presentation = None;
    runner.display.confirm_dialog = None;
    runner.display.help_popup = None;
    for mode in ["none", "live-grid", "plain-grid", "active-behavior"] {
        runner.display.hdmi.mode = mode.into();
        let scene = converted_scene(&runner, false);
        assert_eq!(scene["hdmi"]["mode"], mode);
        assert_eq!(
            scene["hdmi"]["grid"]["rgb"].as_array().unwrap().len(),
            GRID_WIDTH * GRID_HEIGHT * 3
        );
        assert_eq!(
            scene["leds"]["active"].as_array().unwrap().len(),
            GRID_WIDTH * GRID_HEIGHT
        );
        if mode == "none" {
            assert_eq!(scene["hdmi"]["showGridlines"], false);
            assert_eq!(scene["hdmi"]["cycleMeasures"], 4);
            assert!(scene["hdmi"]["grid"]["rgb"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v == 0));
            assert!(scene["hdmi"]["grid"]["active"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v == false));
        } else if mode == "live-grid" {
            assert_eq!(scene["hdmi"]["grid"]["rgb"], scene["leds"]["rgb"]);
            assert_eq!(scene["hdmi"]["grid"]["active"], scene["leds"]["active"]);
        } else {
            assert_eq!(scene["hdmi"]["sourceLayerIndex"], 0, "{mode}");
            assert_eq!(scene["hdmi"]["sourceBehaviorId"], "life", "{mode}");
            let rgb = scene["hdmi"]["grid"]["rgb"].as_array().unwrap();
            let alive = (GRID_HEIGHT - 1 - 1) * GRID_WIDTH + 2;
            assert_eq!(scene["hdmi"]["grid"]["active"][alive], true, "{mode}");
            assert_eq!(
                rgb[alive * 3..alive * 3 + 3],
                [json!(255), json!(255), json!(0)],
                "{mode}"
            );
            let empty = (GRID_HEIGHT - 1) * GRID_WIDTH;
            assert_eq!(scene["hdmi"]["grid"]["active"][empty], false, "{mode}");
            assert_eq!(
                rgb[empty * 3..empty * 3 + 3],
                [json!(0), json!(48), json!(0)],
                "{mode}"
            );
        }
    }
    runner.display.oled_mode = NativeOledMode::Off;
    let sleeping = converted_scene(&runner, false);
    assert_eq!(sleeping["display"]["off"], true);
    assert_eq!(sleeping["display"]["title"], "MENU");
}

#[test]
fn scene_capture_and_conversion_walltime_diagnostic() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let payload: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    runner.apply_config_payload(payload).unwrap();
    let _ = runner
        .capture_presentation_scene(true)
        .unwrap()
        .into_snapshot();
    for audio in [false, true] {
        let mut capture = Duration::ZERO;
        let mut conversion = Duration::ZERO;
        for _ in 0..32 {
            let started = Instant::now();
            let scene = runner.capture_presentation_scene(audio).unwrap();
            capture += started.elapsed();
            let started = Instant::now();
            let result = scene.into_snapshot();
            conversion += started.elapsed();
            assert_eq!(result["settings"].get("instruments").is_some(), audio);
        }
        eprintln!(
            "scene audio={audio} capture_avg_us={} conversion_avg_us={} samples=32",
            capture.as_micros() / 32,
            conversion.as_micros() / 32
        );
    }
}
