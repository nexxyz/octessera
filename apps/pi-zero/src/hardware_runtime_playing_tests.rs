use super::*;

#[test]
fn realtime_activation_rebases_before_the_first_playing_tick() {
    let now = Instant::now();
    let (playback, _runner, _host) = playing_playback();
    let mut scheduler = HardwareRuntimeScheduler::new(now, playback.last_snapshot_revision());

    assert!(scheduler
        .next_runtime_advance(now + Duration::from_secs(1), &playback, None)
        .is_none());
    assert_eq!(
        scheduler
            .next_runtime_advance(
                now + Duration::from_secs(1) + PLAYBACK_TICK,
                &playback,
                None,
            )
            .expect("playing tick should be due")
            .elapsed,
        PLAYBACK_TICK
    );
}

#[test]
fn playing_runtime_keeps_eight_millisecond_ticks_without_legacy_snapshots() {
    let now = Instant::now();
    let mut scheduler = HardwareRuntimeScheduler::new(now, 0);
    let (playback, _runner, _host) = playing_playback();
    let baseline = now + Duration::from_millis(1);

    assert!(matches!(
        scheduler.display_snapshot_message(&playback),
        HostMessage::TransportPulseStep {
            request_snapshot: Some(false),
            ..
        }
    ));

    assert!(scheduler
        .next_runtime_advance(baseline, &playback, None)
        .is_none());
    let first = scheduler
        .next_runtime_advance(baseline + Duration::from_millis(33), &playback, None)
        .expect("playing advance should be due");
    assert!(!first.request_snapshot);
    let first_attempt = scheduler.last_snapshot_attempt_at;
    let next = scheduler
        .next_runtime_advance(baseline + Duration::from_millis(41), &playback, None)
        .expect("next playing advance should be due");
    assert!(!next.request_snapshot);
    assert_eq!(scheduler.last_snapshot_attempt_at, first_attempt);
}

#[test]
fn playing_runtime_refreshes_a_plain_scene_every_thirty_three_milliseconds() {
    let now = Instant::now();
    let (playback, runner, _host) = playing_playback();
    let scheduler = HardwareRuntimeScheduler::new(now, playback.last_snapshot_revision());

    assert!(!scheduler
        .display_snapshot_due(
            now + SNAPSHOT_TICK - Duration::from_nanos(1),
            &runner,
            &playback
        )
        .any());
    assert!(
        scheduler
            .display_snapshot_due(now + SNAPSHOT_TICK, &runner, &playback)
            .continuous
    );
}

#[test]
fn accepted_external_snapshot_rebases_playing_and_continuous_cadence() {
    let now = Instant::now();
    let (mut playback, mut runner, mut host) = playing_playback();
    let initial_revision = playback.last_snapshot_revision();
    let mut scheduler = HardwareRuntimeScheduler::new(now, initial_revision);

    assert!(scheduler
        .next_runtime_advance(now, &playback, None)
        .is_none());
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::StoreError {
                message: "a".repeat(40),
            },
        })
        .expect("long toast should be accepted");
    playback
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({"type": "other"}),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .expect("external snapshot should be accepted");
    let accepted_revision = playback.last_snapshot_revision();
    assert!(accepted_revision > initial_revision);
    let accepted_at = now + PLAYBACK_TICK;
    scheduler.observe_snapshot_revision(accepted_at, initial_revision, accepted_revision);
    assert_eq!(scheduler.last_snapshot_attempt_at, accepted_at);

    let before_playing_due = scheduler
        .next_runtime_advance(
            accepted_at + SNAPSHOT_TICK - Duration::from_nanos(1),
            &playback,
            None,
        )
        .expect("playing tick should be due before snapshot cadence");
    assert!(!before_playing_due.request_snapshot);
    assert!(
        !scheduler
            .display_snapshot_due(
                accepted_at + SNAPSHOT_TICK - Duration::from_nanos(1),
                &runner,
                &playback,
            )
            .continuous
    );
    let after_playing_due = scheduler
        .next_runtime_advance(accepted_at + SNAPSHOT_TICK + PLAYBACK_TICK, &playback, None)
        .expect("next playing tick should be due");
    assert!(!after_playing_due.request_snapshot);
    assert!(
        scheduler
            .display_snapshot_due(accepted_at + SNAPSHOT_TICK, &runner, &playback)
            .continuous
    );
}
