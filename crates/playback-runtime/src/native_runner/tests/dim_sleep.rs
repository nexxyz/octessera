use super::*;
use std::time::{Duration, Instant};

#[test]
pub(crate) fn settings_leds_dimmed_after_dim_timer() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.ui.dim_timer_seconds = 1;
    runner.last_interaction_at = Instant::now() - Duration::from_secs(1);

    assert_eq!(runner.snapshot().unwrap()["settings"]["ledsDimmed"], true);
}

#[test]
pub(crate) fn one_second_dim_and_two_second_sleep_trigger_on_snapshots() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.ui.dim_timer_seconds = 1;
    runner.ui.screen_sleep_seconds = 2;
    runner.last_interaction_at = Instant::now() - Duration::from_millis(1500);

    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["settings"]["ledsDimmed"], true);
    assert_eq!(snapshot["display"]["splash"], "");
    assert_eq!(snapshot["display"]["off"], false);

    runner.last_interaction_at = Instant::now() - Duration::from_secs(2);
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["settings"]["ledsDimmed"], true);
    assert_eq!(snapshot["display"]["splash"], "sleep");
    assert_eq!(snapshot["display"]["off"], false);

    runner.oled_splash_until = Some(Instant::now() - Duration::from_millis(1));
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["display"]["off"], true);
}

#[test]
pub(crate) fn timed_display_deadline_tracks_dim_sleep_and_splash() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.ui.dim_timer_seconds = 1;
    runner.ui.screen_sleep_seconds = 2;
    let interaction = Instant::now();
    runner.last_interaction_at = interaction;

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

    runner.ui.dim_timer_seconds = 0;
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(interaction + Duration::from_secs(2))
    );

    let splash_until = Instant::now() + Duration::from_millis(25);
    runner.oled_mode = NativeOledMode::Splash;
    runner.oled_splash_until = Some(splash_until);
    assert_eq!(
        runner.next_timed_display_snapshot_deadline(),
        Some(splash_until)
    );
}

#[test]
pub(crate) fn input_resets_dim_and_oled_off_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.ui.dim_timer_seconds = 1;
    runner.oled_mode = NativeOledMode::Off;
    runner.last_interaction_at = Instant::now() - Duration::from_secs(2);
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
pub(crate) fn suppressed_input_wake_still_emits_display_snapshot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.oled_mode = NativeOledMode::Off;
    runner.last_interaction_at = Instant::now() - Duration::from_secs(2);

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
    runner.oled_mode = NativeOledMode::Off;

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
    runner.ui.screen_sleep_seconds = 0;
    runner.oled_mode = NativeOledMode::Normal;
    runner.last_interaction_at = Instant::now()
        .checked_sub(Duration::from_secs(3600))
        .unwrap_or_else(Instant::now);

    let snapshot = runner.messages_with_snapshot().unwrap();

    assert_eq!(snapshot_from(&snapshot)["display"]["off"], false);
    assert_eq!(snapshot_from(&snapshot)["display"]["splash"], "");
}
