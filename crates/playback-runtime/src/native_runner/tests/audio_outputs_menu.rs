use super::*;
#[test]
pub(crate) fn usb_menu_edits_payload_with_restart_dialog() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert!(runner.menu.focus_item_key("audioOutputs.usb"));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.snapshot().unwrap()["display"]["title"], "/SYS/Audio");
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["audioOutputs"],
        json!({ "dac": true, "usb": true, "hdmi": false })
    );
    assert_eq!(snapshot_from(&messages)["display"]["title"], "Save Setting");
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
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            jack_audio_required: true,
            ..NativeRunnerConfig::default()
        })
        .unwrap();
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
pub(crate) fn required_jack_policy_device_input_replays_usb_and_hdmi_without_errors() {
    for (audio_optimization, capacity_available) in [
        (AudioOptimization::Latency, false),
        (AudioOptimization::Capacity, true),
    ] {
        for (key, expected) in [
            (
                "audioOutputs.usb",
                json!({ "dac": true, "usb": true, "hdmi": false }),
            ),
            (
                "audioOutputs.hdmi",
                json!({ "dac": true, "usb": false, "hdmi": true }),
            ),
        ] {
            let mut runner = NativeRunner::new(NativeRunnerConfig {
                audio_optimization,
                audio_optimization_capacity_available: capacity_available,
                jack_audio_required: true,
                ..NativeRunnerConfig::default()
            })
            .unwrap();
            let before_revision = runner.config_revision;

            assert!(!runner.menu.focus_item_key("audioOutputs.dac"));
            assert!(runner.menu.focus_item_key(key));
            runner
                .send(HostMessage::DeviceInput {
                    input: json!({ "type": "encoder_press", "id": "main" }),
                    request_snapshot: None,
                })
                .unwrap();
            let messages = runner
                .send(HostMessage::DeviceInput {
                    input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
                    request_snapshot: None,
                })
                .unwrap();

            assert_eq!(
                runner.config_payload()["runtimeConfig"]["audioOutputs"],
                expected
            );
            assert!(runner.audio_outputs.dac());
            assert!(runner.config_dirty);
            assert_eq!(runner.config_revision, before_revision + 1);
            assert!(runner.pending.pending_autosave_payload_due_at.is_none());
            assert_ne!(snapshot_from(&messages)["display"]["title"], "Save Setting");
        }
    }
}
