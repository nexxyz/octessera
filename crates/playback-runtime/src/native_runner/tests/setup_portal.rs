use super::*;
use crate::{RuntimeSetupPortalDisposition, RuntimeSetupPortalErrorCode};

fn portal_status(phase: RuntimeSetupPortalPhase) -> RuntimeStoreResult {
    let (disposition, suffix, error_code) = match phase {
        RuntimeSetupPortalPhase::Starting => {
            (Some(RuntimeSetupPortalDisposition::Accepted), None, None)
        }
        RuntimeSetupPortalPhase::PortalReady => (None, Some("1a2f".into()), None),
        RuntimeSetupPortalPhase::Finalizing | RuntimeSetupPortalPhase::Succeeded => {
            (None, None, None)
        }
        RuntimeSetupPortalPhase::Failed => (
            None,
            None,
            Some(RuntimeSetupPortalErrorCode::OperationFailed),
        ),
        RuntimeSetupPortalPhase::TimedOut => {
            (None, None, Some(RuntimeSetupPortalErrorCode::Unavailable))
        }
        RuntimeSetupPortalPhase::Unsupported => {
            (None, None, Some(RuntimeSetupPortalErrorCode::Unsupported))
        }
    };
    RuntimeStoreResult::SetupPortalStatus {
        status: RuntimeSetupPortalStatus {
            phase,
            disposition,
            portal_suffix: suffix,
            reboot_required: false,
            error_code,
        },
    }
}

fn identified(phase: RuntimeSetupPortalPhase, revision: u64) -> RuntimeStoreResult {
    portal_status(phase).with_identity("setup-1".into(), Some(revision))
}

fn send_main(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn display_lines(messages: &[RunnerMessage]) -> Vec<String> {
    snapshot_from(messages)["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap().to_string())
        .collect()
}

#[test]
pub(crate) fn configure_wifi_replay_confirms_with_one_stop_panic_and_portal_effect() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("system.configureWifi"));
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.current_ppqn_pulse = 96;

    let confirmation = send_main(&mut runner, json!({"type":"encoder_press","id":"main"}));
    assert_eq!(
        snapshot_from(&confirmation)["display"]["title"],
        "Open Wi-Fi Setup"
    );
    assert!(display_lines(&confirmation)
        .iter()
        .any(|line| line.contains("Wi-Fi")));

    let cancelled = send_main(&mut runner, json!({"type":"button_a","pressed":true}));
    assert!(cancelled
        .iter()
        .all(|message| !matches!(message, RunnerMessage::PlatformEffects { .. })));
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);

    let _ = send_main(&mut runner, json!({"type":"encoder_press","id":"main"}));
    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;
    let confirmed = send_main(&mut runner, json!({"type":"encoder_press","id":"main"}));
    let effects = confirmed
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects }
                if effects
                    .iter()
                    .any(|effect| matches!(effect, RuntimePlatformEffect::SetupPortalOpen)) =>
            {
                Some(effects)
            }
            _ => None,
        })
        .expect("setup effects");
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, RuntimePlatformEffect::SetupPortalOpen))
            .count(),
        1
    );
    assert!(effects
        .iter()
        .any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic)));
    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);
    assert_eq!(runner.transport.current_ppqn_pulse, 0);
    assert!(!runner
        .status()
        .transport
        .eq(&RuntimeTransportState::Playing));
    assert_eq!(
        snapshot_from(&confirmed)["display"]["lines"][0],
        "Starting hotspot..."
    );
}

#[test]
pub(crate) fn portal_lifecycle_statuses_keep_compact_actionable_snapshots() {
    for (phase, expected) in [
        (RuntimeSetupPortalPhase::Starting, "Starting hotspot..."),
        (RuntimeSetupPortalPhase::PortalReady, "Octessera Setup 1a2f"),
        (RuntimeSetupPortalPhase::Finalizing, "Applying settings..."),
        (RuntimeSetupPortalPhase::Succeeded, "Setup complete"),
        (RuntimeSetupPortalPhase::Failed, "Setup failed"),
        (RuntimeSetupPortalPhase::TimedOut, "Setup timed out"),
        (
            RuntimeSetupPortalPhase::Unsupported,
            "Not available on desktop",
        ),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.stop_for_setup_portal();
        let messages = runner
            .send(HostMessage::RuntimeResult {
                result: identified(phase.clone(), 1),
            })
            .unwrap();
        let snapshot = snapshot_from(&messages);
        let lines = display_lines(&messages);
        assert!(lines.iter().any(|line| line == expected));
        assert!(lines.len() <= 7);
        assert!(lines.iter().all(|line| line.chars().count() <= 28));
        assert_eq!(
            snapshot["selectedRow"].as_u64(),
            Some((lines.len() - 1) as u64)
        );
        assert!(snapshot["display"]["scrollOffset"].is_null());
        assert!(lines.last().unwrap().starts_with("> "));
    }
}

#[test]
pub(crate) fn already_running_is_presented_as_the_same_starting_lifecycle() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.stop_for_setup_portal();
    let result = RuntimeStoreResult::SetupPortalStatus {
        status: RuntimeSetupPortalStatus {
            phase: RuntimeSetupPortalPhase::Starting,
            disposition: Some(RuntimeSetupPortalDisposition::AlreadyRunning),
            portal_suffix: None,
            reboot_required: false,
            error_code: None,
        },
    }
    .with_identity("setup-1".into(), Some(1));
    let messages = runner.send(HostMessage::RuntimeResult { result }).unwrap();
    assert_eq!(
        snapshot_from(&messages)["display"]["lines"][0],
        "Starting hotspot..."
    );
    assert_eq!(
        runner
            .display
            .setup_portal
            .as_ref()
            .unwrap()
            .status
            .disposition,
        Some(RuntimeSetupPortalDisposition::AlreadyRunning)
    );
}

#[test]
pub(crate) fn portal_hide_suppresses_current_phase_but_reopens_for_completion() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.stop_for_setup_portal();
    let _ = send_main(&mut runner, json!({"type":"button_a","pressed":true}));
    assert!(!runner.display.setup_portal.as_ref().unwrap().visible);

    let hidden = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::Starting, 1),
        })
        .unwrap();
    assert_eq!(snapshot_from(&hidden)["display"]["title"], "MENU");

    let ready = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::PortalReady, 1),
        })
        .unwrap();
    assert_eq!(snapshot_from(&ready)["display"]["title"], "Wi-Fi Setup");

    let _ = send_main(&mut runner, json!({"type":"button_a","pressed":true}));
    let succeeded = runner
        .send(HostMessage::RuntimeResult {
            result: identified(RuntimeSetupPortalPhase::Succeeded, 1),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&succeeded)["display"]["lines"][0],
        "Setup complete"
    );
    let _ = send_main(&mut runner, json!({"type":"button_a","pressed":true}));
    assert!(runner.display.setup_portal.is_none());
}

#[test]
pub(crate) fn portal_identity_filter_rejects_stale_and_out_of_order_results() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.stop_for_setup_portal();
    runner
        .apply_store_result(identified(RuntimeSetupPortalPhase::PortalReady, 4))
        .unwrap();
    runner
        .apply_store_result(identified(RuntimeSetupPortalPhase::Starting, 4))
        .unwrap();
    assert_eq!(
        runner.display.setup_portal.as_ref().unwrap().status.phase,
        RuntimeSetupPortalPhase::PortalReady
    );
    runner
        .apply_store_result(portal_status(RuntimeSetupPortalPhase::Failed))
        .unwrap();
    assert_eq!(
        runner.display.setup_portal.as_ref().unwrap().status.phase,
        RuntimeSetupPortalPhase::PortalReady
    );
    runner
        .apply_store_result(identified(RuntimeSetupPortalPhase::Succeeded, 3))
        .unwrap();
    assert_eq!(
        runner.display.setup_portal.as_ref().unwrap().status.phase,
        RuntimeSetupPortalPhase::PortalReady
    );
}

#[test]
pub(crate) fn setup_modal_has_priority_over_other_presentations() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.stop_for_setup_portal();
    runner.display.usb_sd_transfer_modal = Some(NativeUsbSdTransferModal {
        title: "USB".into(),
        lines: vec!["transfer".into()],
    });
    runner.display.system_info_modal = Some(NativeSystemInfoModal::loading());
    runner.display.help_popup = Some(NativeHelpPopup {
        title: "Help".into(),
        lines: vec!["help".into()],
        scroll: 0,
    });
    runner.display.runtime_error_presentation = Some(NativeRuntimeErrorPresentation {
        title: "Runtime Error".into(),
        lines: vec!["bad".into()],
    });
    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["display"]["title"], "Wi-Fi Setup");
    assert!(!snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "USB"));
}
