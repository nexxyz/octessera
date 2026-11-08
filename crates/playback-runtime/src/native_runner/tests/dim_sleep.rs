use super::*;
use std::time::{Duration, Instant};

#[test]
pub(crate) fn settings_leds_dimmed_after_dim_timer() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.ui.dim_timer_seconds = 1;
    runner.display.last_interaction_at = Instant::now() - Duration::from_secs(1);

    assert_eq!(runner.snapshot().unwrap()["settings"]["ledsDimmed"], true);
}

#[test]
pub(crate) fn one_second_dim_and_two_second_sleep_trigger_on_snapshots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.ui.dim_timer_seconds = 1;
    runner.display.ui.screen_sleep_seconds = 2;
    runner.display.last_interaction_at = Instant::now() - Duration::from_millis(1500);

    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["settings"]["ledsDimmed"], true);
    assert_eq!(snapshot["display"]["splash"], "");
    assert_eq!(snapshot["display"]["off"], false);

    runner.display.last_interaction_at = Instant::now() - Duration::from_secs(2);
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["settings"]["ledsDimmed"], true);
    assert_eq!(snapshot["display"]["splash"], "sleep");
    assert_eq!(snapshot["display"]["off"], false);

    runner.display.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["display"]["off"], true);
}

#[test]
pub(crate) fn timed_display_deadline_tracks_dim_sleep_and_splash() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.ui.dim_timer_seconds = 1;
    runner.display.ui.screen_sleep_seconds = 2;
    let interaction = Instant::now();
    runner.display.last_interaction_at = interaction;

    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(interaction + Duration::from_secs(1))
    );
    assert_eq!(
        runner.next_timed_display_snapshot_deadline_after(Some(
            interaction + Duration::from_millis(1500)
        )),
        Some(interaction + Duration::from_secs(2))
    );

    runner.display.ui.dim_timer_seconds = 0;
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(interaction + Duration::from_secs(2))
    );

    let splash_until = Instant::now() + Duration::from_millis(25);
    runner.display.oled_mode = NativeOledMode::Splash;
    runner.display.oled_splash_until = Some(splash_until);
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(splash_until)
    );
}

#[test]
pub(crate) fn input_resets_dim_and_oled_off_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.ui.dim_timer_seconds = 1;
    runner.display.oled_mode = NativeOledMode::Off;
    runner.display.last_interaction_at = Instant::now() - Duration::from_secs(2);
    assert_eq!(runner.snapshot().unwrap()["settings"]["ledsDimmed"], true);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
            request_snapshot: Some(true),
        })
        .unwrap();
    let snapshot = snapshot_from(&messages);

    assert_eq!(snapshot["settings"]["ledsDimmed"], false);
    assert_eq!(snapshot["display"]["off"], false);
}

#[test]
pub(crate) fn overdue_normal_display_input_does_not_trigger_sleep_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.ui.screen_sleep_seconds = 1;
    runner.display.oled_mode = NativeOledMode::Normal;
    runner.display.last_interaction_at = Instant::now() - Duration::from_secs(2);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: Some(true),
        })
        .unwrap();
    let snapshot = snapshot_from(&messages);

    assert_eq!(snapshot["display"]["splash"], "");
    assert_eq!(snapshot["display"]["off"], false);
    assert_ne!(snapshot["display"]["toast"], "Going to sleep");
    assert_eq!(runner.menu.state.cursor, 1);
}

#[test]
pub(crate) fn fresh_startup_timed_snapshots_reach_sleep_without_input() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.ui.screen_sleep_seconds = 1;
    runner.display.oled_splash_until = Some(Instant::now() + Duration::from_secs(1));

    let messages = runner.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_from(&messages)["display"]["splash"], "startup");

    runner.display.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));
    let messages = runner.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_from(&messages)["display"]["splash"], "");
    assert_eq!(
        snapshot_from(&messages)["display"]["toast"],
        "Help: Sh+Fn+Main"
    );

    runner.display.last_interaction_at = Instant::now() - Duration::from_secs(2);
    let messages = runner.messages_with_snapshot().unwrap();
    assert_eq!(snapshot_from(&messages)["display"]["splash"], "sleep");
    assert_eq!(
        snapshot_from(&messages)["display"]["toast"],
        "Going to sleep"
    );
}

#[test]
pub(crate) fn startup_splash_completion_resets_sleep_deadline() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.ui.screen_sleep_seconds = 1;
    runner.display.last_interaction_at = Instant::now() - Duration::from_secs(2);
    runner.display.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));

    let messages = runner.messages_with_snapshot().unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "");
    assert_eq!(display["off"], false);

    let messages = runner.messages_with_snapshot().unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "");
    assert_eq!(display["off"], false);

    runner.display.last_interaction_at = Instant::now() - Duration::from_secs(2);
    let messages = runner.messages_with_snapshot().unwrap();
    let display = &snapshot_from(&messages)["display"];
    assert_eq!(display["splash"], "sleep");
    assert_eq!(display["off"], false);
}

#[test]
pub(crate) fn suppressed_input_wake_still_emits_display_snapshot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.oled_mode = NativeOledMode::Off;
    runner.display.last_interaction_at = Instant::now() - Duration::from_secs(2);

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: Some(false),
        })
        .unwrap();

    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["off"], false);
    assert_eq!(runner.menu.state.cursor, 0);
}

#[test]
pub(crate) fn runtime_and_transport_messages_do_not_wake_oled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.oled_mode = NativeOledMode::Off;

    let runtime_messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiStatus {
                ok: true,
                message: None,
                selected_out_id: None,
                selected_in_id: None,
            },
        })
        .unwrap();
    assert_eq!(snapshot_from(&runtime_messages)["display"]["off"], true);

    let transport_messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    assert_eq!(snapshot_from(&transport_messages)["display"]["off"], true);
}

#[test]
pub(crate) fn screen_sleep_zero_prevents_off_and_sleep() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.ui.screen_sleep_seconds = 0;
    runner.display.oled_mode = NativeOledMode::Normal;
    runner.display.last_interaction_at = Instant::now()
        .checked_sub(Duration::from_secs(3600))
        .unwrap_or_else(Instant::now);

    let snapshot = runner.messages_with_snapshot().unwrap();

    assert_eq!(snapshot_from(&snapshot)["display"]["off"], false);
    assert_eq!(snapshot_from(&snapshot)["display"]["splash"], "");
}

fn setup_portal_status(phase: RuntimeSetupPortalPhase, revision: u64) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::SetupPortalStatus {
            status: RuntimeSetupPortalStatus {
                phase,
                disposition: None,
                portal_suffix: None,
                portal_passphrase: None,
                reboot_required: false,
                error_code: None,
            },
        }
        .with_identity("setup-1".into(), Some(revision)),
    }
}

#[test]
pub(crate) fn setup_portal_phase_change_wakes_oled_and_restarts_sleep() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.display.ui.dim_timer_seconds = 1;
    runner.display.ui.screen_sleep_seconds = 60;
    runner.stop_for_setup_portal();
    runner
        .send(setup_portal_status(RuntimeSetupPortalPhase::PortalReady, 1))
        .unwrap();
    runner.display.oled_mode = NativeOledMode::Off;
    let stale_interaction = runner.display.last_interaction_at - Duration::from_secs(120);
    runner.display.last_interaction_at = stale_interaction;

    let before = Instant::now();
    let messages = runner
        .send(setup_portal_status(RuntimeSetupPortalPhase::Finalizing, 2))
        .unwrap();
    let after = Instant::now();

    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["off"], false);
    assert_eq!(snapshot["settings"]["ledsDimmed"], false);
    let woke_at = runner.display.last_interaction_at;
    assert!(before <= woke_at && woke_at <= after);

    runner.display.oled_mode = NativeOledMode::Off;
    runner.display.last_interaction_at = stale_interaction;
    let repeated = runner
        .send(setup_portal_status(RuntimeSetupPortalPhase::Finalizing, 3))
        .unwrap();
    assert_eq!(snapshot_from(&repeated)["display"]["off"], true);
    assert_eq!(runner.display.last_interaction_at, stale_interaction);
}

#[test]
pub(crate) fn setup_success_wakes_into_sys_info_popup_without_moving_the_menu() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.skip_startup_splash();
    runner.display.ui.screen_sleep_seconds = 60;
    assert!(runner.menu.focus_item_key("system.configureWifi"));
    runner.stop_for_setup_portal();
    runner
        .send(setup_portal_status(RuntimeSetupPortalPhase::Finalizing, 1))
        .unwrap();
    runner.display.oled_mode = NativeOledMode::Off;
    let label_before = runner.test_current_menu_label();

    let before = Instant::now();
    let messages = runner
        .send(setup_portal_status(RuntimeSetupPortalPhase::Succeeded, 2))
        .unwrap();
    let woke_at = runner.display.last_interaction_at;
    assert!(before <= woke_at && woke_at <= Instant::now());
    let snapshot = snapshot_from(&messages);
    assert_eq!(snapshot["display"]["off"], false);
    assert_eq!(snapshot["display"]["title"], "Sys. Info");
    assert_eq!(runner.test_current_menu_label(), label_before);

    let closed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: Some(true),
        })
        .unwrap();
    assert!(runner.display.system_info_modal.is_none());
    assert!(runner.display.confirm_dialog.is_none());
    assert_eq!(runner.test_current_menu_label(), label_before);
    assert_eq!(
        snapshot_from(&closed)["display"]["title"],
        "/SYS/Connections/Wi-Fi"
    );
}
