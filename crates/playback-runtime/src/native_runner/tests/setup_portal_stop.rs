use super::setup_portal::{display_lines, identified, send_main};
use super::*;

fn has_effect(messages: &[RunnerMessage], wanted: &RuntimePlatformEffect) -> bool {
    messages.iter().any(|message| {
        matches!(message, RunnerMessage::PlatformEffects { effects } if effects.contains(wanted))
    })
}

fn open_portal_from_menu(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    assert!(runner.menu.focus_item_key("system.configureWifi"));
    let _ = send_main(runner, json!({"type":"encoder_press","id":"main"}));
    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;
    send_main(runner, json!({"type":"encoder_press","id":"main"}))
}

#[test]
pub(crate) fn stop_hotspot_closes_the_card_and_stops_the_portal_once() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert!(has_effect(
        &open_portal_from_menu(&mut runner),
        &RuntimePlatformEffect::SetupPortalOpen
    ));
    let ready = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::PortalReady, 1),
        })
        .unwrap();
    assert_eq!(display_lines(&ready).last().unwrap(), "Stop hotspot");

    let closed = send_main(&mut runner, json!({"type":"encoder_press","id":"main"}));
    assert!(has_effect(
        &closed,
        &RuntimePlatformEffect::SetupPortalClose
    ));
    assert_ne!(snapshot_from(&closed)["display"]["title"], "Wi-Fi Setup");
    assert!(runner.setup_portal_closing());

    let reopened = open_portal_from_menu(&mut runner);
    assert!(!has_effect(
        &reopened,
        &RuntimePlatformEffect::SetupPortalOpen
    ));
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Wi-Fi Setup is closing")
    );

    let stopped = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::Failed, 1),
        })
        .unwrap();
    assert_ne!(snapshot_from(&stopped)["display"]["title"], "Wi-Fi Setup");
    assert!(!runner.display.setup_portal.as_ref().unwrap().visible);
    assert!(!runner.setup_portal_closing());

    let reopened = open_portal_from_menu(&mut runner);
    assert!(has_effect(
        &reopened,
        &RuntimePlatformEffect::SetupPortalOpen
    ));
    assert_eq!(
        snapshot_from(&reopened)["display"]["lines"][0],
        "Starting hotspot..."
    );
}

#[test]
pub(crate) fn back_also_stops_and_a_late_success_still_shows_sys_info() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.stop_for_setup_portal();
    let closed = send_main(&mut runner, json!({"type":"button_a","pressed":true}));
    assert!(has_effect(
        &closed,
        &RuntimePlatformEffect::SetupPortalClose
    ));

    let hidden = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::Starting, 1),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&hidden)["display"]["title"],
        runner.active_layer_title()
    );
    let finalizing = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::Finalizing, 1),
        })
        .unwrap();
    assert_ne!(
        snapshot_from(&finalizing)["display"]["title"],
        "Wi-Fi Setup"
    );

    let succeeded = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::Succeeded, 1),
        })
        .unwrap();
    assert_eq!(snapshot_from(&succeeded)["display"]["title"], "Sys. Info");
    assert!(!runner.display.setup_portal.as_ref().unwrap().visible);
    let dismissed = send_main(&mut runner, json!({"type":"button_a","pressed":true}));
    assert!(!has_effect(
        &dismissed,
        &RuntimePlatformEffect::SetupPortalClose
    ));
    assert!(runner.display.system_info_modal.is_none());

    let repeated = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::Succeeded, 1),
        })
        .unwrap();
    assert_ne!(snapshot_from(&repeated)["display"]["title"], "Sys. Info");
    assert!(runner.display.system_info_modal.is_none());
}

#[test]
pub(crate) fn close_while_applying_only_hides_and_a_failure_still_shows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.stop_for_setup_portal();
    let applying = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::Finalizing, 1),
        })
        .unwrap();
    assert_eq!(display_lines(&applying).last().unwrap(), "Close");
    let hidden = send_main(&mut runner, json!({"type":"encoder_press","id":"main"}));
    assert!(!has_effect(
        &hidden,
        &RuntimePlatformEffect::SetupPortalClose
    ));
    assert_ne!(snapshot_from(&hidden)["display"]["title"], "Wi-Fi Setup");

    let failed = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::Failed, 1),
        })
        .unwrap();
    assert_eq!(snapshot_from(&failed)["display"]["title"], "Wi-Fi Setup");
    assert_eq!(display_lines(&failed)[0], "Setup failed");
}

#[test]
pub(crate) fn a_wifi_chip_crash_during_stop_hotspot_keeps_the_card_visible() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert!(has_effect(
        &open_portal_from_menu(&mut runner),
        &RuntimePlatformEffect::SetupPortalOpen
    ));
    runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::PortalReady, 1),
        })
        .unwrap();
    let closed = send_main(&mut runner, json!({"type":"encoder_press","id":"main"}));
    assert!(has_effect(
        &closed,
        &RuntimePlatformEffect::SetupPortalClose
    ));
    assert_ne!(snapshot_from(&closed)["display"]["title"], "Wi-Fi Setup");

    let crashed = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SetupPortalStatus {
                status: RuntimeSetupPortalStatus {
                    phase: RuntimeSetupPortalPhase::Failed,
                    disposition: None,
                    portal_suffix: None,
                    portal_passphrase: None,
                    reboot_required: false,
                    error_code: Some(crate::RuntimeSetupPortalErrorCode::RadioRestartRequired),
                },
            }
            .with_identity("setup-1".into(), Some(1)),
        })
        .unwrap();
    assert_eq!(snapshot_from(&crashed)["display"]["title"], "Wi-Fi Setup");
    assert_eq!(
        display_lines(&crashed),
        vec!["Wi-Fi chip stopped", "Restart the device", "Close"]
    );

    let dismissed = send_main(&mut runner, json!({"type":"encoder_press","id":"main"}));
    assert!(!has_effect(
        &dismissed,
        &RuntimePlatformEffect::SetupPortalClose
    ));
    assert_ne!(snapshot_from(&dismissed)["display"]["title"], "Wi-Fi Setup");
}
