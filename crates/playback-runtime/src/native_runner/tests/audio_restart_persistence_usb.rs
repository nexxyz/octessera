use super::audio_restart_persistence::{enter_edit, press, turn};
use super::*;

fn save_setting_payload(
    runner: &mut NativeRunner,
    key: &str,
    delta: i32,
    request_id: &str,
) -> Value {
    enter_edit(runner, key);
    turn(runner, delta);
    press(runner);
    turn(runner, 1);
    let messages = press(runner);
    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                let RuntimePlatformEffect::StoreSaveDefault { payload, mode } = effect else {
                    return None;
                };
                (mode.as_deref() == Some("restart-setting")).then_some(payload.clone())
            }),
            _ => None,
        })
        .expect("restart setting save effect");
    let revision = runner.restart_settings.pending_write_revision().unwrap();
    runner.register_default_write_request(request_id, Some(revision));
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                    ok: true,
                    is_auto: None,
                }),
                request_id: request_id.into(),
                revision: Some(revision),
            },
        })
        .unwrap();
    payload
}

fn reload_default_payload(payload: Value) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            },
        })
        .unwrap();
    runner
}

#[test]
fn usb_midi_setting_survives_unrelated_restart_setting_save_and_reload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let usb_payload = save_setting_payload(&mut runner, "usb.midiOutEnabled", 1, "usb-save");
    assert_eq!(usb_payload["runtimeConfig"]["usb"]["midiOutEnabled"], true);

    let mut runner = reload_default_payload(usb_payload);
    assert_eq!(
        runner.test_config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
        true
    );
    let buffer_payload = save_setting_payload(
        &mut runner,
        "sound.audioOutputBufferFrames",
        1,
        "buffer-save",
    );

    let reloaded = reload_default_payload(buffer_payload);
    assert_eq!(
        reloaded.test_config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
        true
    );
}

#[test]
fn host_role_stages_gadget_outputs_off_and_saves_the_compound_setting() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        usb_data_role_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.audio_outputs = AudioOutputSet::from_flags(true, true, false).unwrap();
    runner.usb_midi_out_enabled = true;
    runner.menu.rebuild(runner.menu_config());

    enter_edit(&mut runner, "usb.dataRole");
    turn(&mut runner, 1);
    let messages = press(&mut runner);

    assert_eq!(runner.usb_data_role, UsbDataRole::Host);
    assert!(!runner.audio_outputs.usb());
    assert!(!runner.usb_midi_out_enabled);
    assert!(!runner.menu.focus_item_key("audioOutputs.usb"));
    assert!(!runner.menu.focus_item_key("usb.midiOutEnabled"));
    assert!(!runner.menu.focus_item_key("usb.sdTransferStart"));
    assert!(runner.menu.focus_item_key("usb.sdTransferStop"));
    assert_eq!(
        runner.display.confirm_dialog.as_ref().unwrap().lines,
        vec![
            "Restart required.",
            "Before Host reboot:",
            "Unplug computer USB.",
            "Audio/MIDI/SD2 off.",
        ]
    );
    assert_eq!(
        runner.display.confirm_dialog.as_ref().unwrap().options,
        vec!["Cancel", "Save this setting", "Save everything"]
    );
    assert_eq!(snapshot_from(&messages)["display"]["title"], "Save Setting");

    turn(&mut runner, 1);
    let messages = press(&mut runner);
    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                let RuntimePlatformEffect::StoreSaveDefault { payload, mode } = effect else {
                    return None;
                };
                (mode.as_deref() == Some("restart-setting")).then_some(payload)
            }),
            _ => None,
        })
        .expect("USB Role restart save effect");
    assert_eq!(payload["runtimeConfig"]["usb"]["dataRole"], "host");
    assert_eq!(payload["runtimeConfig"]["usb"]["midiOutEnabled"], false);
    assert_eq!(payload["runtimeConfig"]["audioOutputs"]["usb"], false);
}

#[test]
fn legacy_usb_role_defaults_to_gadget() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        usb_data_role_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let mut host_payload = runner.config_payload();
    host_payload["runtimeConfig"]["usb"]["dataRole"] = json!("host");
    runner.apply_config_payload(host_payload).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["usb"]
        .as_object_mut()
        .unwrap()
        .remove("dataRole");
    runner
        .apply_config_payload(legacy_payload(payload))
        .unwrap();
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["usb"]["dataRole"],
        "gadget"
    );
}

#[test]
fn host_payload_rejects_usb_audio_without_mutating_runner() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["usb"]["dataRole"] = json!("host");
    payload["runtimeConfig"]["audioOutputs"]["usb"] = json!(true);
    assert_rejected_without_byte_changes(&mut runner, payload);
}

#[test]
fn host_role_save_is_blocked_while_sd2_transfer_is_active() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        usb_data_role_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.display.usb_sd_transfer_modal = Some(NativeUsbSdTransferModal {
        title: "SD2 Transfer".into(),
        lines: vec!["Transfer active".into()],
    });
    runner.menu.set_enum_value_for_key("usb.dataRole", "Host");
    let mut expected_menu = runner.menu.clone();
    expected_menu.set_enum_value_for_key("usb.dataRole", "Gadget");
    let before_config = runner.config_payload();
    let before_role = runner.usb_data_role;
    let before_audio_outputs = runner.audio_outputs;
    let before_usb_midi = runner.usb_midi_out_enabled;
    let before_dirty = runner.config_dirty;
    let before_revision = runner.config_revision;
    let before_dirty_revision = runner.dirty_revision;
    let before_restart_settings = runner.restart_settings.clone();
    assert!(runner.apply_runtime_menu_key_fast("usb.dataRole").unwrap());
    assert!(runner.display.confirm_dialog.is_none());
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Stop SD2 transfer before selecting Host"
    );
    assert!(!runner.restart_settings.is_save_choice());
    assert_eq!(runner.usb_data_role, before_role);
    assert_eq!(runner.audio_outputs, before_audio_outputs);
    assert_eq!(runner.usb_midi_out_enabled, before_usb_midi);
    assert_eq!(runner.config_dirty, before_dirty);
    assert_eq!(runner.config_revision, before_revision);
    assert_eq!(runner.dirty_revision, before_dirty_revision);
    assert_eq!(runner.config_payload(), before_config);
    assert_eq!(runner.menu, expected_menu);
    assert_eq!(runner.restart_settings, before_restart_settings);
    assert!(!runner.outbox.has_platform_effects());
    assert!(!runner.outbox.has_audio_commands());
}

#[test]
fn selecting_gadget_does_not_restore_host_disabled_outputs() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        usb_data_role_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.audio_outputs = AudioOutputSet::from_flags(true, true, false).unwrap();
    runner.usb_midi_out_enabled = true;
    runner.menu.rebuild(runner.menu_config());

    runner.menu.set_enum_value_for_key("usb.dataRole", "Host");
    assert!(runner.apply_runtime_menu_key_fast("usb.dataRole").unwrap());
    runner.display.confirm_dialog = None;
    runner.restart_settings.cancel();
    runner.menu.set_enum_value_for_key("usb.dataRole", "Gadget");
    assert!(runner.apply_runtime_menu_key_fast("usb.dataRole").unwrap());

    assert_eq!(runner.usb_data_role, UsbDataRole::Gadget);
    assert!(!runner.audio_outputs.usb());
    assert!(!runner.usb_midi_out_enabled);
}
