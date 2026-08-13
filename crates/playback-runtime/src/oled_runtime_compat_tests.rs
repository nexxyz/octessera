use super::oled_runtime_fixtures::{present, snapshot, status};
use super::support::FakeHost;
use crate::protocol::{base64_encode_count, reset_base64_encode_count};
use crate::{
    CoreRunner, HostMessage, PlaybackRuntime, RunnerMessage, RuntimeConfig, RuntimeStatus,
    RuntimeStatusState, RuntimeTransportState, SyncSource,
};
use serde_json::Value;
use std::collections::VecDeque;

#[derive(Default)]
struct ChangedSnapshotRunner {
    titles: VecDeque<String>,
}

impl CoreRunner for ChangedSnapshotRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let responds_to_pulse = matches!(
            message,
            HostMessage::TransportPulseStep { .. }
                | HostMessage::MidiRealtimeStart
                | HostMessage::RuntimeResult { .. }
        );
        let Some(title) = responds_to_pulse.then(|| self.titles.pop_front()).flatten() else {
            return Ok(Vec::new());
        };
        Ok(vec![
            RunnerMessage::Snapshot {
                snapshot: snapshot(&title),
            },
            RunnerMessage::RuntimeStatus {
                status: playing_status(),
            },
        ])
    }
}

fn assert_delivered_frames_cover_snapshots(
    messages: &[RunnerMessage],
    accepted_revisions: &mut Vec<u64>,
) {
    for message in messages {
        match message {
            RunnerMessage::OledFrame { revision, .. } => accepted_revisions.push(*revision),
            RunnerMessage::Snapshot { snapshot } => {
                let Some(revision) = snapshot.get("oledFrameRevision").and_then(Value::as_u64)
                else {
                    continue;
                };
                assert!(
                    accepted_revisions.contains(&revision),
                    "snapshot references an undelivered OLED frame revision {revision}"
                );
            }
            _ => {}
        }
    }
}

fn assert_final_presentation(messages: &[RunnerMessage], title: &str, revision: u64) {
    assert!(matches!(
        messages,
        [
            RunnerMessage::OledFrame { revision: frame_revision, .. },
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { .. }
        ] if *frame_revision == revision
            && snapshot["display"]["title"] == title
            && snapshot["oledFrameRevision"] == revision
    ));
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(message, RunnerMessage::OledFrame { .. }))
            .count(),
        1
    );
}

fn playing_status() -> RuntimeStatus {
    RuntimeStatus {
        state: RuntimeStatusState::Running,
        transport: RuntimeTransportState::Playing,
        current_ppqn_pulse: 0,
        pending_resync: false,
        sync_source: SyncSource::Internal,
        message: None,
        error: None,
    }
}

fn seed_playing_runtime() -> (PlaybackRuntime, FakeHost, ChangedSnapshotRunner) {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let _ = present(&mut runtime, snapshot("initial"));
    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::RuntimeStatus {
                status: playing_status(),
            }],
            &mut host,
        )
        .unwrap();
    (
        runtime,
        host,
        ChangedSnapshotRunner {
            titles: VecDeque::from(["first discarded".into(), "latest discarded".into()]),
        },
    )
}

fn assert_republished_latest_frame(
    runtime: &mut PlaybackRuntime,
    runner: &mut ChangedSnapshotRunner,
    host: &mut FakeHost,
) {
    reset_base64_encode_count();
    let output = runtime
        .dispatch_runner_messages(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: snapshot("latest discarded"),
                },
                RunnerMessage::RuntimeStatus {
                    status: playing_status(),
                },
            ],
            runner,
            host,
        )
        .unwrap();
    assert!(matches!(
        output.messages.as_slice(),
        [
            RunnerMessage::OledFrame { revision, pixels, .. },
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { .. }
        ] if snapshot["oledFrameRevision"] == *revision
            && *revision == runtime.oled_frame_revision()
            && runtime.last_oled_frame() == Some(pixels.as_slice())
    ));
    assert_eq!(base64_encode_count(), 0);

    let _wire = serde_json::to_string(&output.messages).unwrap();
    assert_eq!(base64_encode_count(), 1);

    let unchanged = runtime
        .dispatch_runner_messages(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: snapshot("latest discarded"),
                },
                RunnerMessage::RuntimeStatus {
                    status: playing_status(),
                },
            ],
            runner,
            host,
        )
        .unwrap();
    assert!(unchanged
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::OledFrame { .. })));
}

#[test]
fn no_output_advance_requeues_latest_frame_once() {
    let (mut runtime, mut host, mut runner) = seed_playing_runtime();
    runtime.advance(500, &mut runner, &mut host).unwrap();
    runtime.advance(500, &mut runner, &mut host).unwrap();
    assert_republished_latest_frame(&mut runtime, &mut runner, &mut host);
}

#[test]
fn no_output_advance_duration_requeues_latest_frame_once() {
    let (mut runtime, mut host, mut runner) = seed_playing_runtime();
    runtime
        .advance_duration(
            std::time::Duration::from_millis(500),
            &mut runner,
            &mut host,
        )
        .unwrap();
    runtime
        .advance_duration(
            std::time::Duration::from_millis(500),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_republished_latest_frame(&mut runtime, &mut runner, &mut host);
}

#[test]
fn no_output_midi_realtime_requeues_latest_frame_once() {
    let (mut runtime, mut host, mut runner) = seed_playing_runtime();
    runtime
        .handle_midi_realtime_bytes(&[0xfa], &mut runner, &mut host)
        .unwrap();
    runtime
        .handle_midi_realtime_bytes(&[0xfa], &mut runner, &mut host)
        .unwrap();
    assert_republished_latest_frame(&mut runtime, &mut runner, &mut host);
}

#[test]
fn no_output_ingest_requeues_latest_frame_and_preserves_follow_ups() {
    let (mut runtime, mut host, mut runner) = seed_playing_runtime();
    let follow_ups = runtime
        .ingest_runner_messages(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: snapshot("first discarded"),
                },
                RunnerMessage::RuntimeStatus {
                    status: playing_status(),
                },
            ],
            &mut host,
        )
        .unwrap();
    assert!(follow_ups.is_empty());
    let _ = runtime
        .ingest_runner_messages(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: snapshot("latest discarded"),
                },
                RunnerMessage::RuntimeStatus {
                    status: playing_status(),
                },
            ],
            &mut host,
        )
        .unwrap();
    assert_republished_latest_frame(&mut runtime, &mut runner, &mut host);
}

#[test]
fn no_output_calls_without_frames_do_not_publish_an_unchanged_frame() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let mut runner = ChangedSnapshotRunner::default();
    let _ = present(&mut runtime, snapshot("initial"));

    runtime.advance(500, &mut runner, &mut host).unwrap();
    runtime
        .handle_midi_realtime_bytes(&[], &mut runner, &mut host)
        .unwrap();
    runtime
        .ingest_runner_messages(
            vec![RunnerMessage::RuntimeStatus { status: status() }],
            &mut host,
        )
        .unwrap();

    let output = runtime
        .dispatch_runner_messages(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: snapshot("initial"),
                },
                RunnerMessage::RuntimeStatus { status: status() },
            ],
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(output
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::OledFrame { .. })));
}

#[test]
fn platform_follow_up_dispatch_and_compatibility_requeue_preserve_oled_pairs() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let mut runner = ChangedSnapshotRunner {
        titles: VecDeque::from(["effect changed".into()]),
    };
    let mut delivered = present(&mut runtime, snapshot("initial"));
    let mut accepted_revisions = Vec::new();
    assert_delivered_frames_cover_snapshots(&delivered, &mut accepted_revisions);

    let effect_output = runtime
        .ingest_runner_messages_with_output(
            vec![RunnerMessage::PlatformEffects {
                effects: vec![crate::RuntimePlatformEffect::StoreListPresets],
            }],
            &mut host,
        )
        .unwrap();
    assert_eq!(
        host.effects,
        vec![crate::RuntimePlatformEffect::StoreListPresets]
    );
    assert!(matches!(
        effect_output.messages.as_slice(),
        [
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { .. }
        ] if snapshot["oledFrameRevision"] == 1
    ));
    assert_eq!(effect_output.follow_ups.len(), 1);

    for follow_up in effect_output.follow_ups {
        let output = runtime
            .dispatch_host_message(follow_up, &mut runner, &mut host)
            .unwrap();
        assert_final_presentation(&output.messages, "effect changed", 2);
        assert_delivered_frames_cover_snapshots(&output.messages, &mut accepted_revisions);
        delivered.extend(output.messages);
    }
    assert_eq!(accepted_revisions, vec![1, 2]);

    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let mut runner = ChangedSnapshotRunner {
        titles: VecDeque::from(["compat latest".into()]),
    };
    let initial = present(&mut runtime, snapshot("initial"));
    let mut accepted_revisions = Vec::new();
    assert_delivered_frames_cover_snapshots(&initial, &mut accepted_revisions);

    let follow_ups = runtime
        .ingest_runner_messages(
            vec![
                RunnerMessage::PlatformEffects {
                    effects: vec![crate::RuntimePlatformEffect::StoreListPresets],
                },
                RunnerMessage::Snapshot {
                    snapshot: snapshot("obsolete"),
                },
                RunnerMessage::RuntimeStatus { status: status() },
                RunnerMessage::Snapshot {
                    snapshot: snapshot("compat latest"),
                },
                RunnerMessage::RuntimeStatus { status: status() },
            ],
            &mut host,
        )
        .unwrap();
    assert_eq!(follow_ups.len(), 1);
    assert!(matches!(
        follow_ups.as_slice(),
        [HostMessage::RuntimeResult { .. }]
    ));

    let output = runtime
        .dispatch_host_message(
            follow_ups.into_iter().next().unwrap(),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_final_presentation(&output.messages, "compat latest", 3);
    assert_delivered_frames_cover_snapshots(&output.messages, &mut accepted_revisions);
    assert_eq!(accepted_revisions, vec![1, 3]);
    assert!(output.messages.iter().all(|message| {
        !matches!(message, RunnerMessage::Snapshot { snapshot } if snapshot["display"]["title"] == "obsolete")
    }));
}
