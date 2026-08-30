use super::*;
use crate::oled_frame::TOAST_RECT;

#[test]
pub(crate) fn usb_menu_edits_payload_with_save_reboot_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("audioOutputs.usb"));
    runner.menu.state.editing = true;

    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": true, "usb": true, "hdmi": false })
    );
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "Audio: Save / Reb"
    );
}

#[test]
pub(crate) fn final_audio_output_off_is_refused_without_dirtying_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("audioOutputs.dac"));
    runner.menu.state.editing = true;
    runner.menu.turn(-1);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": true, "usb": false, "hdmi": false })
    );
    assert!(!runner.config_dirty);
    assert_eq!(
        runner.snapshot().unwrap()["display"]["toast"],
        "Keep one audio ou"
    );
}

#[test]
pub(crate) fn device_input_refuses_final_output_for_each_single_output_set() {
    for (initial, key) in [
        (
            AudioOutputSet::from_flags(true, false, false).unwrap(),
            "audioOutputs.dac",
        ),
        (
            AudioOutputSet::from_flags(false, true, false).unwrap(),
            "audioOutputs.usb",
        ),
        (
            AudioOutputSet::from_flags(false, false, true).unwrap(),
            "audioOutputs.hdmi",
        ),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.audio_outputs = initial;
        runner.menu.rebuild(runner.menu_config());
        let before_payload = runner.config_payload();
        let before_revision = runner.config_revision;
        let before_dirty_revision = runner.dirty_revision;
        let before_dirty = runner.config_dirty;
        let before_autosave = runner.pending.pending_autosave_payload_due_at;
        let before_fast_marks = runner.fast_autosave_marks;

        assert!(runner.menu.focus_item_key(key));
        let _ = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();
        let messages = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();

        assert_eq!(runner.config_payload(), before_payload);
        assert_eq!(runner.config_revision, before_revision);
        assert_eq!(runner.dirty_revision, before_dirty_revision);
        assert_eq!(runner.config_dirty, before_dirty);
        assert_eq!(
            runner.pending.pending_autosave_payload_due_at,
            before_autosave
        );
        assert_eq!(runner.fast_autosave_marks, before_fast_marks);
        assert_eq!(
            runner.menu.value_for_key("audioOutputs.dac"),
            Some(initial.dac().to_string())
        );
        assert_eq!(
            runner.menu.value_for_key("audioOutputs.usb"),
            Some(initial.usb().to_string())
        );
        assert_eq!(
            runner.menu.value_for_key("audioOutputs.hdmi"),
            Some(initial.hdmi().to_string())
        );
        assert!(messages.iter().any(|message| matches!(
            message,
            RunnerMessage::Snapshot { snapshot }
                if snapshot["display"]["toast"]
                    .as_str()
                    .is_some_and(|toast| !toast.is_empty() && toast.chars().count() <= TOAST_RECT.columns())
        )));
    }
}

#[test]
pub(crate) fn audio_output_device_input_replays_apply_each_toggle_atomically() {
    for (initial, key, delta, expected) in [
        (
            AudioOutputSet::default(),
            "audioOutputs.usb",
            1,
            json!({ "dac": true, "usb": true, "hdmi": false }),
        ),
        (
            AudioOutputSet::default(),
            "audioOutputs.hdmi",
            1,
            json!({ "dac": true, "usb": false, "hdmi": true }),
        ),
        (
            AudioOutputSet::from_flags(true, true, false).unwrap(),
            "audioOutputs.dac",
            -1,
            json!({ "dac": false, "usb": true, "hdmi": false }),
        ),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.audio_outputs = initial;
        runner.menu.rebuild(runner.menu_config());
        assert!(runner.menu.focus_item_key(key));
        let _ = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();
        let _ = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();
        assert_eq!(
            runner.config_payload()["runtimeConfig"]["audioOutputs"],
            expected
        );
        assert!(runner.config_dirty);
    }
}

#[test]
pub(crate) fn usb_apply_reboot_is_confirmed_and_emits_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.audio_outputs = AudioOutputSet::from_flags(true, true, false).unwrap();
    runner.usb_midi_out_enabled = true;
    assert!(runner.menu.focus_item_key("audio.applyReboot"));

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["title"], "Confirm Audio");
    let lines = snapshot["display"]["lines"].as_array().unwrap();
    assert!(lines.iter().any(|line| line == "> Cancel"));
    assert!(lines.iter().any(|line| line == "  Save / Reboot"));

    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    let effects = messages.iter().find_map(|message| match message {
        RunnerMessage::PlatformEffects { effects } => Some(effects),
        _ => None,
    });
    assert!(matches!(effects, Some(effects) if effects.len() == 1));
    let Some(RuntimePlatformEffect::ApplyDeviceConfigReboot { payload }) = effects.unwrap().first()
    else {
        panic!("expected device config reboot effect");
    };
    assert_eq!(
        payload["runtimeConfig"]["audioOutputs"],
        json!({ "dac": true, "usb": true, "hdmi": false })
    );
    assert_eq!(
        payload["runtimeConfig"]["usb"],
        json!({ "midiOutEnabled": true })
    );
}
