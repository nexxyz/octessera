use super::*;
use playback_runtime::{
    CoreRunner, HostAdapter, MusicalEvent, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand,
    RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::json;

#[derive(Default)]
struct TestHost {
    midi_messages: Vec<Vec<u8>>,
}

#[derive(Default)]
struct TestRunner;

impl CoreRunner for TestRunner {
    fn send(&mut self, _message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        Ok(Vec::new())
    }
}

impl HostAdapter for TestHost {
    fn handle_musical_event(&mut self, _event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        _request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        Ok(Vec::new())
    }

    fn handle_audio_command(
        &mut self,
        _command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        self.midi_messages.push(bytes.to_vec());
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
}

fn playing_playback() -> (PlaybackRuntime, NativeRunner, TestHost) {
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(playback_runtime::NativeRunnerConfig::default())
        .expect("native runner should initialize");
    runner.skip_startup_splash();
    let mut host = TestHost::default();
    playback
        .dispatch_host_message(HostMessage::MidiRealtimeStart, &mut runner, &mut host)
        .expect("start should dispatch");
    (playback, runner, host)
}

fn snapshot_playback() -> (PlaybackRuntime, NativeRunner, TestHost) {
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(playback_runtime::NativeRunnerConfig::default())
        .expect("native runner should initialize");
    let mut host = TestHost::default();
    playback
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({"type": "other"}),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .expect("snapshot input should dispatch");
    (playback, runner, host)
}

#[test]
fn stopped_runtime_uses_fifty_millisecond_maintenance() {
    let now = Instant::now();
    let mut scheduler = HardwareRuntimeScheduler::new(now, 0);
    let playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());

    assert!(scheduler
        .next_runtime_advance(now + MAINTENANCE_TICK - Duration::from_nanos(1), &playback)
        .is_none());
    assert_eq!(
        scheduler
            .next_runtime_advance(now + MAINTENANCE_TICK, &playback)
            .expect("maintenance should be due")
            .elapsed,
        MAINTENANCE_TICK
    );
}

#[test]
fn display_snapshot_request_uses_configured_sync_source() {
    let scheduler = HardwareRuntimeScheduler::new(Instant::now(), 0);
    let playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig {
        sync_source: SyncSource::External,
        ..playback_runtime::RuntimeConfig::default()
    });
    let HostMessage::TransportPulseStep {
        pulses,
        source,
        request_snapshot,
        ..
    } = scheduler.display_snapshot_message(&playback)
    else {
        panic!("expected transport pulse snapshot request");
    };
    assert_eq!(pulses, 0);
    assert_eq!(source, SyncSource::External);
    assert_eq!(request_snapshot, Some(true));
}

#[test]
fn rejected_display_request_keeps_a_bounded_retry_deadline() {
    let now = Instant::now();
    let mut scheduler = HardwareRuntimeScheduler::new(now, 0);
    let runner = NativeRunner::new(playback_runtime::NativeRunnerConfig::default()).unwrap();
    let deadline = runner
        .next_timed_display_snapshot_deadline_after(Some(now))
        .expect("startup display should have a timed deadline");

    assert!(scheduler.display_snapshot_due(deadline, &runner).one_shot);
    scheduler.record_snapshot_attempt(
        deadline,
        DisplaySnapshotDue {
            one_shot: true,
            continuous: false,
        },
        0,
        0,
    );
    assert!(!scheduler.display_snapshot_due(deadline, &runner).any());
    assert!(
        scheduler
            .display_snapshot_due(deadline + SNAPSHOT_RETRY_DELAY, &runner)
            .one_shot
    );
}

#[test]
fn realtime_activation_rebases_before_the_first_playing_tick() {
    let now = Instant::now();
    let (playback, _runner, _host) = playing_playback();
    let mut scheduler = HardwareRuntimeScheduler::new(now, playback.last_snapshot_revision());

    assert!(scheduler
        .next_runtime_advance(now + Duration::from_secs(1), &playback)
        .is_none());
    assert_eq!(
        scheduler
            .next_runtime_advance(now + Duration::from_secs(1) + PLAYBACK_TICK, &playback)
            .expect("playing tick should be due")
            .elapsed,
        PLAYBACK_TICK
    );
}

#[test]
fn playing_runtime_requests_one_snapshot_per_thirty_three_milliseconds() {
    let now = Instant::now();
    let mut scheduler = HardwareRuntimeScheduler::new(now, 0);
    let (playback, _runner, _host) = playing_playback();
    let baseline = now + Duration::from_millis(1);

    assert!(scheduler
        .next_runtime_advance(baseline, &playback)
        .is_none());
    let first = scheduler
        .next_runtime_advance(baseline + Duration::from_millis(33), &playback)
        .expect("playing advance should be due");
    assert!(first.request_snapshot);
    let first_attempt = baseline + Duration::from_millis(33);
    scheduler.record_snapshot_attempt(first_attempt, DisplaySnapshotDue::default(), 1, 1);
    let next = scheduler
        .next_runtime_advance(baseline + Duration::from_millis(41), &playback)
        .expect("next playing advance should be due");
    assert!(!next.request_snapshot);
    assert_eq!(scheduler.last_snapshot_attempt_at, first_attempt);
}

#[test]
fn accepted_external_snapshot_rebases_playing_and_continuous_cadence() {
    let now = Instant::now();
    let (mut playback, mut runner, mut host) = playing_playback();
    let initial_revision = playback.last_snapshot_revision();
    let mut scheduler = HardwareRuntimeScheduler::new(now, initial_revision);

    assert!(scheduler.next_runtime_advance(now, &playback).is_none());
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
        )
        .expect("playing tick should be due before snapshot cadence");
    assert!(!before_playing_due.request_snapshot);
    assert!(
        !scheduler
            .display_snapshot_due(
                accepted_at + SNAPSHOT_TICK - Duration::from_nanos(1),
                &runner,
            )
            .continuous
    );
    let after_playing_due = scheduler
        .next_runtime_advance(accepted_at + SNAPSHOT_TICK + PLAYBACK_TICK, &playback)
        .expect("next playing tick should be due");
    assert!(after_playing_due.request_snapshot);
    assert!(
        scheduler
            .display_snapshot_due(accepted_at + SNAPSHOT_TICK, &runner)
            .continuous
    );
}

#[test]
fn scheduled_midi_idle_rebases_before_a_new_eight_millisecond_tick() {
    let now = Instant::now();
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig {
        sync_source: SyncSource::External,
        midi_out_enabled: true,
        ..playback_runtime::RuntimeConfig::default()
    });
    let mut runner = TestRunner;
    let mut host = TestHost::default();
    let mut scheduler = HardwareRuntimeScheduler::new(now, 0);

    playback
        .ingest_runner_messages(
            vec![RunnerMessage::MidiEvents {
                events: vec![MusicalEvent::NoteOn {
                    channel: 1,
                    note: 64,
                    velocity: 90,
                    duration_ms: Some(8),
                }],
            }],
            &mut host,
        )
        .expect("initial MIDI note should be accepted");
    assert!(playback.has_scheduled_midi());
    assert!(scheduler.next_runtime_advance(now, &playback).is_none());

    let first_tick_at = now + PLAYBACK_TICK;
    let first_advance = scheduler
        .next_runtime_advance(first_tick_at, &playback)
        .expect("scheduled MIDI should receive its first tick");
    assert_eq!(first_advance.elapsed, PLAYBACK_TICK);
    playback
        .advance_duration_with_output(first_advance.elapsed, &mut runner, &mut host)
        .expect("final scheduled note-off should flush");
    scheduler.record_runtime_advance_complete(first_tick_at, &playback);
    assert!(!playback.has_scheduled_midi());
    assert_eq!(
        host.midi_messages,
        vec![vec![0x91, 64, 90], vec![0x81, 64, 0]]
    );

    playback
        .ingest_runner_messages(
            vec![RunnerMessage::MidiEvents {
                events: vec![MusicalEvent::NoteOn {
                    channel: 1,
                    note: 65,
                    velocity: 91,
                    duration_ms: Some(30),
                }],
            }],
            &mut host,
        )
        .expect("new short MIDI note should be accepted");
    let idle_rebase_at = first_tick_at + Duration::from_secs(1);
    assert!(scheduler
        .next_runtime_advance(idle_rebase_at, &playback)
        .is_none());
    let restarted = scheduler
        .next_runtime_advance(idle_rebase_at + PLAYBACK_TICK, &playback)
        .expect("restarted MIDI runtime should tick after eight milliseconds");
    assert_eq!(restarted.elapsed, PLAYBACK_TICK);
    playback
        .advance_duration_with_output(restarted.elapsed, &mut runner, &mut host)
        .expect("new MIDI note should remain active");
    assert_eq!(
        host.midi_messages,
        vec![vec![0x91, 64, 90], vec![0x81, 64, 0], vec![0x91, 65, 91],]
    );

    let note_off_at = idle_rebase_at + Duration::from_millis(30);
    let note_off_advance = scheduler
        .next_runtime_advance(note_off_at, &playback)
        .expect("new MIDI note-off should be due");
    playback
        .advance_duration_with_output(note_off_advance.elapsed, &mut runner, &mut host)
        .expect("new MIDI note-off should flush at its due time");
    assert_eq!(
        host.midi_messages,
        vec![
            vec![0x91, 64, 90],
            vec![0x81, 64, 0],
            vec![0x91, 65, 91],
            vec![0x81, 65, 0],
        ]
    );
}

#[test]
fn external_sync_keeps_runtime_event_driven() {
    let now = Instant::now();
    let mut scheduler = HardwareRuntimeScheduler::new(now, 0);
    let playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig {
        sync_source: SyncSource::External,
        ..playback_runtime::RuntimeConfig::default()
    });

    let advance = scheduler
        .next_runtime_advance(now + MAINTENANCE_TICK, &playback)
        .expect("stopped maintenance should be due");
    assert!(!advance.request_snapshot);
    assert!(!runtime_tick_needed(&playback));
}

#[test]
fn publication_revision_dedupes_and_retries_after_rejection() {
    let now = Instant::now();
    let mut scheduler = HardwareRuntimeScheduler::new(now, 0);
    let (playback, _runner, _host) = snapshot_playback();
    let revision = playback.last_snapshot_revision();

    assert!(revision > 0);
    assert!(scheduler.snapshot_publication_due(now, &playback));
    scheduler.record_snapshot_publication_attempt(now);
    assert!(!scheduler
        .snapshot_publication_due(now + SNAPSHOT_TICK - Duration::from_nanos(1), &playback));
    assert!(scheduler.snapshot_publication_due(now + SNAPSHOT_TICK, &playback));
    scheduler.record_snapshot_publication_attempt(now + SNAPSHOT_TICK);
    scheduler.record_snapshot_publication_accepted(revision);
    assert!(!scheduler.snapshot_publication_due(now + SNAPSHOT_TICK, &playback));
}

#[test]
fn revision_observation_separates_attempt_and_acceptance_times() {
    let now = Instant::now();
    let mut scheduler = HardwareRuntimeScheduler::new(now, 1);
    let unchanged = now + Duration::from_millis(10);
    scheduler.record_snapshot_attempt(unchanged, DisplaySnapshotDue::default(), 1, 1);
    assert_eq!(scheduler.last_snapshot_attempt_at, unchanged);
    assert_eq!(scheduler.last_accepted_snapshot_at, now);

    let changed = now + Duration::from_millis(20);
    scheduler.record_snapshot_attempt(changed, DisplaySnapshotDue::default(), 1, 2);
    assert_eq!(scheduler.last_snapshot_attempt_at, changed);
    assert_eq!(scheduler.last_accepted_snapshot_at, changed);
    assert_eq!(scheduler.last_accepted_snapshot_revision, 2);
}

#[test]
fn stopped_input_keeps_native_snapshot_request_and_playing_input_suppresses_it() {
    let stopped = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let message = prepare_dispatch_message(
        &stopped,
        HostMessage::DeviceInput {
            input: json!({"type": "other"}),
            request_snapshot: None,
        },
    );
    assert!(matches!(
        message,
        HostMessage::DeviceInput {
            request_snapshot: None,
            ..
        }
    ));

    let (playing, _runner, _host) = playing_playback();
    let message = prepare_dispatch_message(
        &playing,
        HostMessage::DeviceInput {
            input: json!({"type": "other"}),
            request_snapshot: None,
        },
    );
    assert!(matches!(
        message,
        HostMessage::DeviceInput {
            request_snapshot: Some(false),
            ..
        }
    ));
}

#[test]
fn scheduler_sleep_never_exceeds_eight_milliseconds() {
    let now = Instant::now();
    let scheduler = HardwareRuntimeScheduler::new(now, 0);
    let playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let runner = NativeRunner::new(playback_runtime::NativeRunnerConfig::default()).unwrap();

    assert!(scheduler.sleep_duration(now, &playback, &runner) <= SLEEP_MAX);
}

#[test]
fn playing_snapshot_deadline_waits_for_the_next_eight_millisecond_tick() {
    let now = Instant::now();
    let (playback, _runner, _host) = playing_playback();
    let mut scheduler = HardwareRuntimeScheduler::new(now, playback.last_snapshot_revision());
    let mut runner = NativeRunner::new(playback_runtime::NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.messages_with_snapshot().unwrap();
    assert!(scheduler.next_runtime_advance(now, &playback).is_none());
    for offset in [8, 16, 24, 32] {
        assert!(scheduler
            .next_runtime_advance(now + Duration::from_millis(offset), &playback)
            .is_some());
    }

    assert_eq!(
        scheduler.sleep_duration(now + Duration::from_millis(33), &playback, &runner),
        Duration::from_millis(7)
    );
}
