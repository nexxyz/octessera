use super::*;

fn board_runner() -> NativeRunner {
    NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap()
}

fn input(runner: &mut NativeRunner, input: Value) {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
}

fn set_wifi(runner: &mut NativeRunner, on: bool) {
    assert!(runner.menu.focus_item_key("wifi.enabled"));
    input(runner, json!({ "type": "encoder_press", "id": "main" }));
    input(
        runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": if on { 1 } else { -1 } }),
    );
    input(runner, json!({ "type": "encoder_press", "id": "main" }));
}

#[test]
fn wifi_switch_toggles_a_saved_device_setting_that_stays_out_of_portable_patches() {
    let mut runner = board_runner();
    assert!(runner.wifi_enabled());

    set_wifi(&mut runner, false);
    assert!(!runner.wifi_enabled());
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Wi-Fi off: no SSH"
    );
    let payload = runner.config_payload();
    assert_eq!(payload["runtimeConfig"]["wifi"]["enabled"], false);
    assert!(
        portable_patch_projection(&payload).unwrap()["runtimeConfig"]
            .get("wifi")
            .is_none()
    );

    let mut restarted = board_runner();
    restarted.apply_config_payload(payload).unwrap();
    assert!(!restarted.wifi_enabled());

    set_wifi(&mut restarted, true);
    assert!(restarted.wifi_enabled());
    assert_eq!(
        restarted.display.toast.as_ref().unwrap().message,
        "Wi-Fi on"
    );
}

#[test]
fn wifi_switch_and_setup_portal_are_board_only() {
    let mut desktop = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(!desktop.menu.focus_item_key("wifi.enabled"));
    assert!(!desktop.menu.focus_item_key("system.configureWifi"));
    assert!(board_runner().menu.focus_item_key("system.configureWifi"));
}
