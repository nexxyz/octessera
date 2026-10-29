use super::audio_restart_persistence::{enter_edit, press, turn};
use super::*;
use crate::RuntimePlatformRequest;

fn effects(messages: &[RunnerMessage]) -> Vec<RuntimePlatformEffect> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn applied_payload(messages: &[RunnerMessage]) -> Option<Value> {
    effects(messages)
        .into_iter()
        .find_map(|effect| match effect {
            RuntimePlatformEffect::ApplyDeviceConfig { payload } => Some(payload),
            _ => None,
        })
}

fn saved_payload(messages: &[RunnerMessage]) -> Option<Value> {
    effects(messages)
        .into_iter()
        .find_map(|effect| match effect {
            RuntimePlatformEffect::StoreSaveSystem { payload } => Some(payload),
            _ => None,
        })
}

fn commit_setting(runner: &mut NativeRunner, key: &str, delta: i32) -> Vec<RunnerMessage> {
    enter_edit(runner, key);
    turn(runner, delta);
    press(runner)
}

fn save_setting_payload(
    runner: &mut NativeRunner,
    key: &str,
    delta: i32,
    request_id: &str,
) -> Value {
    let messages = commit_setting(runner, key, delta);
    let payload = saved_payload(&messages).expect("System save effect");
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreSaveSystem {
            payload: payload.clone(),
        },
        request_id.into(),
        None,
    );
    runner.register_platform_request(&request);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(request_id.into(), None),
        })
        .unwrap();
    payload
}

fn live_midi_out_enabled(runner: &NativeRunner) -> bool {
    runner
        .last_published_runtime_config
        .as_ref()
        .expect("runtime config should be published")
        .midi_out_enabled
}

fn board_runner(usb_data_role_available: bool) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        usb_data_role_available,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.midi_enabled = true;
    runner.menu.rebuild(runner.menu_config());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "other" }),
            request_snapshot: None,
        })
        .unwrap();
    runner
}

fn toast(runner: &NativeRunner) -> String {
    runner.display.toast.as_ref().unwrap().message.clone()
}

fn reload_system_payload(payload: Value) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        "reload-system".into(),
        None,
    );
    runner.register_platform_request(&request);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult {
                payload: Some(payload),
            }
            .with_identity(request.request_id, request.revision),
        })
        .unwrap();
    runner
}

#[test]
fn usb_midi_setting_survives_another_device_setting_save_and_reload() {
    let mut runner = board_runner(false);
    let usb_payload = save_setting_payload(&mut runner, "usb.midiOutEnabled", 1, "usb-save");
    assert_eq!(usb_payload["runtimeConfig"]["usb"]["midiOutEnabled"], true);

    let mut runner = reload_system_payload(usb_payload);
    assert_eq!(
        runner.test_config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
        true
    );
    let hdmi_payload = save_setting_payload(&mut runner, "audioOutputs.hdmi", 1, "hdmi-save");

    let reloaded = reload_system_payload(hdmi_payload);
    assert_eq!(
        reloaded.test_config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
        true
    );
}

#[test]
fn each_device_setting_saves_and_applies_live_without_a_dialog() {
    for (key, path) in [
        ("audioOutputs.usb", ["audioOutputs", "usb"]),
        ("audioOutputs.hdmi", ["audioOutputs", "hdmi"]),
        ("usb.midiOutEnabled", ["usb", "midiOutEnabled"]),
    ] {
        let mut runner = board_runner(false);
        let messages = commit_setting(&mut runner, key, 1);

        let saved = saved_payload(&messages).expect("System save effect");
        let applied = applied_payload(&messages).expect("live device apply");
        assert_eq!(applied, saved, "{key}");
        assert_eq!(applied["runtimeConfig"][path[0]][path[1]], true, "{key}");
        assert!(runner.display.confirm_dialog.is_none(), "{key}");
        assert_eq!(toast(&runner), "Applying...", "{key}");
    }
}

#[test]
fn apply_results_show_the_host_status_toast() {
    for (ok, message) in [
        (true, "USB ready"),
        (false, "USB apply failed, reboot to apply"),
    ] {
        let mut runner = board_runner(false);
        let _ = commit_setting(&mut runner, "usb.midiOutEnabled", 1);
        runner
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::DeviceConfigStatus {
                    ok,
                    message: message.into(),
                },
            })
            .unwrap();
        assert_eq!(toast(&runner), message);
        assert!(runner.display.confirm_dialog.is_none());
    }
}

#[test]
fn turning_a_device_setting_back_before_confirming_applies_nothing() {
    let mut runner = board_runner(false);
    enter_edit(&mut runner, "audioOutputs.hdmi");
    turn(&mut runner, 1);
    turn(&mut runner, -1);
    let messages = press(&mut runner);
    assert!(applied_payload(&messages).is_none());
    assert!(saved_payload(&messages).is_none());
}

#[test]
fn load_system_applies_only_changed_device_settings_live() {
    let mut runner = board_runner(false);
    let mut system = split_system_patch_documents(&runner.config_payload())
        .unwrap()
        .system;
    for usb_audio in [false, true] {
        system["runtimeConfig"]["audioOutputs"]["usb"] = json!(usb_audio);
        let request = RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreLoadSystem,
            format!("load-{usb_audio}"),
            None,
        );
        runner.register_platform_request(&request);
        let messages = runner
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadSystemResult {
                    payload: Some(system.clone()),
                }
                .with_identity(request.request_id, request.revision),
            })
            .unwrap();
        assert_eq!(applied_payload(&messages).is_some(), usb_audio);
    }
}

#[test]
fn usb_midi_changes_the_live_midi_route_at_once() {
    let mut runner = board_runner(false);
    assert!(!live_midi_out_enabled(&runner));

    let _ = save_setting_payload(&mut runner, "usb.midiOutEnabled", 1, "usb-enable-save");
    assert!(live_midi_out_enabled(&runner));

    let saved = save_setting_payload(&mut runner, "usb.midiOutEnabled", -1, "usb-disable-save");
    assert_eq!(saved["runtimeConfig"]["usb"]["midiOutEnabled"], false);
    assert!(!live_midi_out_enabled(&runner));
}

#[test]
fn host_role_turns_gadget_outputs_off_and_applies_them_together() {
    let mut runner = board_runner(true);
    runner.audio_outputs = AudioOutputSet::from_flags(true, true, false).unwrap();
    runner.usb_midi_out_enabled = true;
    runner.menu.rebuild(runner.menu_config());

    let messages = commit_setting(&mut runner, "usb.dataRole", 1);

    assert_eq!(runner.usb_data_role, UsbDataRole::Host);
    assert!(!runner.audio_outputs.usb());
    assert!(!runner.usb_midi_out_enabled);
    assert!(!runner.menu.focus_item_key("audioOutputs.usb"));
    assert!(!runner.menu.focus_item_key("usb.midiOutEnabled"));
    assert!(!runner.menu.focus_item_key("usb.sdTransferStart"));
    assert!(runner.menu.focus_item_key("usb.sdTransferStop"));
    assert!(runner.menu.focus_item_key("midi.output.none"));
    assert!(runner.display.confirm_dialog.is_none());
    let payload = applied_payload(&messages).expect("USB Role live apply");
    assert_eq!(payload["runtimeConfig"]["usb"]["dataRole"], "host");
    assert_eq!(payload["runtimeConfig"]["usb"]["midiOutEnabled"], false);
    assert_eq!(payload["runtimeConfig"]["audioOutputs"]["usb"], false);
}

#[test]
fn scrolling_usb_role_through_host_and_back_keeps_usb_audio_and_midi() {
    let mut runner = board_runner(true);
    runner.audio_outputs = AudioOutputSet::from_flags(true, true, false).unwrap();
    runner.usb_midi_out_enabled = true;
    runner.menu.rebuild(runner.menu_config());

    enter_edit(&mut runner, "usb.dataRole");
    turn(&mut runner, 1);
    turn(&mut runner, -1);
    let messages = press(&mut runner);

    assert_eq!(runner.usb_data_role, UsbDataRole::Gadget);
    assert!(runner.audio_outputs.usb());
    assert!(runner.usb_midi_out_enabled);
    assert!(applied_payload(&messages).is_none());
    assert!(saved_payload(&messages).is_none());
    assert!(runner.pending.system_persistence.dirty_revision.is_none());
}

#[test]
fn unversioned_usb_role_defaults_to_gadget() {
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
        .apply_config_payload(unversioned_payload(payload))
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
fn host_role_is_refused_while_sd2_transfer_is_active() {
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
    assert_eq!(toast(&runner), "Stop SD2 transfer before selecting Host");
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

    let _ = commit_setting(&mut runner, "usb.dataRole", 1);
    let messages = commit_setting(&mut runner, "usb.dataRole", -1);

    assert_eq!(runner.usb_data_role, UsbDataRole::Gadget);
    assert!(!runner.audio_outputs.usb());
    assert!(!runner.usb_midi_out_enabled);
    let payload = applied_payload(&messages).expect("Gadget live apply");
    assert_eq!(payload["runtimeConfig"]["usb"]["dataRole"], "gadget");
}

#[test]
fn orange_usb_device_disable_restores_host_midi_selections() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        audio_optimization_capacity_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.midi_outputs = vec![MidiPort {
        id: "name:Host Out".into(),
        name: "Host Out".into(),
    }];
    runner.midi_inputs = vec![MidiPort {
        id: "name:Host In".into(),
        name: "Host In".into(),
    }];

    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["midi"]["enabled"] = json!(true);
    payload["runtimeConfig"]["midi"]["outId"] = json!("name:Host Out");
    payload["runtimeConfig"]["midi"]["inId"] = json!("name:Host In");
    payload["runtimeConfig"]["usb"]["midiOutEnabled"] = json!(true);
    runner.apply_config_payload(payload).unwrap();

    assert!(runner
        .menu
        .item_for_key("midi.output.name:Host Out")
        .is_some());
    assert!(runner
        .menu
        .item_for_key("midi.input.name:Host In")
        .is_some());
    assert!(runner.menu.item_for_key("usb.midiOutEnabled").is_some());
    assert_eq!(
        runner.selected_midi_output_id.as_deref(),
        Some("name:Host Out")
    );
    assert_eq!(
        runner.selected_midi_input_id.as_deref(),
        Some("name:Host In")
    );

    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["usb"]["midiOutEnabled"] = json!(false);
    runner.apply_config_payload(payload).unwrap();
    let reloaded = reload_system_payload(
        split_system_patch_documents(&runner.config_payload())
            .unwrap()
            .system,
    );

    assert_eq!(
        reloaded.selected_midi_output_id.as_deref(),
        Some("name:Host Out")
    );
    assert_eq!(
        reloaded.selected_midi_input_id.as_deref(),
        Some("name:Host In")
    );
}

#[test]
fn inactive_midi_retains_host_ids_through_usb_status_and_reload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        audio_optimization_capacity_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["midi"]["enabled"] = json!(false);
    payload["runtimeConfig"]["midi"]["outId"] = json!("name:Host Out");
    payload["runtimeConfig"]["midi"]["inId"] = json!("name:Host In");
    payload["runtimeConfig"]["usb"]["midiOutEnabled"] = json!(true);
    runner.apply_config_payload(payload).unwrap();

    let effects = runner.outbox.drain_platform_effects();
    assert!(effects.contains(&RuntimePlatformEffect::MidiSelectOutput {
        id: Some("name:Host Out".into())
    }));
    assert!(effects.contains(&RuntimePlatformEffect::MidiSelectInput {
        id: Some("name:Host In".into())
    }));

    for _ in 0..2 {
        let messages = runner
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::MidiStatus {
                    ok: true,
                    message: None,
                    selected_out_id: Some("name:Host Out".into()),
                    selected_in_id: Some("name:Host In".into()),
                },
            })
            .unwrap();
        assert!(!messages
            .iter()
            .any(|message| matches!(message, RunnerMessage::MidiEvents { .. })));
    }
    assert!(!runner.midi_enabled);
    assert_eq!(
        runner.selected_midi_output_id.as_deref(),
        Some("name:Host Out")
    );
    assert_eq!(
        runner.selected_midi_input_id.as_deref(),
        Some("name:Host In")
    );

    let baseline = runner.config_payload();
    let mut runner = reload_system_payload(split_system_patch_documents(&baseline).unwrap().system);
    let saved = save_setting_payload(&mut runner, "usb.midiOutEnabled", -1, "inactive-usb-save");
    assert_eq!(saved["runtimeConfig"]["midi"]["outId"], "name:Host Out");
    assert_eq!(saved["runtimeConfig"]["midi"]["inId"], "name:Host In");
    assert_eq!(saved["runtimeConfig"]["usb"]["midiOutEnabled"], false);

    let reloaded = reload_system_payload(saved);
    assert!(!reloaded.midi_enabled);
    assert!(!reloaded.usb_midi_out_enabled);
    assert_eq!(
        reloaded.selected_midi_output_id.as_deref(),
        Some("name:Host Out")
    );
    assert_eq!(
        reloaded.selected_midi_input_id.as_deref(),
        Some("name:Host In")
    );
}
