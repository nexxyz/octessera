use super::support::FakeHost;
use crate::{
    HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage,
    RuntimeConfig, RuntimeIngest, RuntimeTransportState, SyncSource,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn default_playback() -> (PlaybackRuntime, NativeRunner, FakeHost, Instant) {
    let payload: Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    assert_eq!(payload["runtimeConfig"]["bpm"], 120);
    assert_eq!(payload["runtimeConfig"]["midi"]["syncMode"], "internal");
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let start = Instant::now();
    runner.test_set_display_time(start);
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let startup = runner.messages_with_snapshot().unwrap();
    runtime
        .dispatch_runner_messages(startup, &mut runner, &mut host)
        .unwrap();
    for pressed in [true, false] {
        runtime
            .dispatch_host_message(
                HostMessage::DeviceInput {
                    input: json!({ "type": "button_s", "pressed": pressed }),
                    request_snapshot: None,
                },
                &mut runner,
                &mut host,
            )
            .unwrap();
    }
    assert_eq!(
        runtime.last_status().unwrap().transport,
        RuntimeTransportState::Playing
    );
    host.musical_events.clear();
    host.audio_commands.clear();
    (runtime, runner, host, start)
}

fn snapshot(output: &RuntimeIngest) -> Option<&Value> {
    output.messages.iter().find_map(|message| match message {
        RunnerMessage::Snapshot { snapshot } => Some(snapshot),
        _ => None,
    })
}

#[test]
fn presentation_scene_public_api_is_sendable_without_runner_reference() {
    fn accepts_send<T: Send>(_scene: &T) {}
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let scene: crate::PresentationScene = runner.capture_next_presentation_scene().unwrap();
    accepts_send(&scene);
    assert!(scene.into_snapshot()["settings"]["instruments"].is_array());
}

fn assert_due_presentation<'a>(output: &'a RuntimeIngest, runtime: &PlaybackRuntime) -> &'a Value {
    let presented = snapshot(output).expect("visible transition needs its full snapshot");
    let revision = presented["oledFrameRevision"]
        .as_u64()
        .expect("OLED revision");
    assert_eq!(runtime.oled_frame_revision(), revision);
    assert!(output.messages.iter().any(|message| matches!(message,
        RunnerMessage::OledFrame { revision: frame_revision, .. } if *frame_revision == revision)));
    assert!(matches!(
        output.messages.last(),
        Some(RunnerMessage::RuntimeStatus { .. })
    ));
    presented
}

#[test]
fn paired_default_patch_snapshot_and_status_present_only_once() {
    let (mut runtime, mut runner, mut host, start) = default_playback();
    runtime.test_enable_dispatch_profile();
    runner.test_set_display_time(start + Duration::from_millis(91));
    runtime.request_next_snapshot();
    let output = runtime
        .advance_duration_with_output(Duration::from_millis(8), &mut runner, &mut host)
        .unwrap();
    let shown = snapshot(&output).expect("explicit full snapshot must be presented");
    let revision = shown["oledFrameRevision"].as_u64().unwrap();
    assert!(revision > 0);
    assert_eq!(runtime.oled_frame_revision(), revision);
    assert!(matches!(
        output.messages.last(),
        Some(RunnerMessage::RuntimeStatus { .. })
    ));
    assert_eq!(runtime.test_received_snapshots(), 1);
    assert_eq!(
        runtime.test_refresh_counts(),
        (1, 0),
        "one native Snapshot followed by Status must not rebuild the same OLED twice"
    );
}

#[test]
fn ordinary_default_patch_status_does_not_republish_unchanged_oled() {
    let (mut runtime, mut runner, mut host, start) = default_playback();
    runner.test_set_display_time(start + Duration::from_millis(91));
    runtime.request_next_snapshot();
    runtime
        .advance_duration_with_output(Duration::from_millis(8), &mut runner, &mut host)
        .unwrap();
    runtime.test_enable_dispatch_profile();
    let mut ordinary = None;
    for _ in 0..24 {
        let before_pulse = runtime.last_status().unwrap().current_ppqn_pulse;
        let revision = runtime.oled_frame_revision();
        let before_snapshots = runtime.test_received_snapshots();
        let output = runtime
            .advance_duration_with_output(Duration::from_millis(8), &mut runner, &mut host)
            .unwrap();
        if runtime.last_status().unwrap().current_ppqn_pulse > before_pulse
            && runtime.oled_frame_revision() == revision
            && runtime.test_received_snapshots() == before_snapshots
        {
            ordinary = Some(output);
            break;
        }
    }
    let output = ordinary.expect("default patch must have an ordinary status-only pulse");
    assert!(output
        .messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::RuntimeStatus { .. })));
    assert!(
        output.messages.iter().all(|message| !matches!(
            message,
            RunnerMessage::Snapshot { .. } | RunnerMessage::OledFrame { .. }
        )),
        "unchanged ordinary status must not clone and republish the full OLED"
    );
    assert_eq!(
        runtime.test_refresh_counts().1,
        0,
        "ordinary status-only pulse must not rebuild the OLED"
    );
}

#[test]
fn default_patch_due_beat_and_event_dot_onset_and_expiry_reach_oled() {
    let (mut runtime, mut runner, mut host, start) = default_playback();
    let original_title = runtime.last_snapshot().unwrap()["display"]["title"].clone();
    runner.test_set_display_time(start + Duration::from_millis(91));
    let beat = runtime
        .advance_duration_with_output(Duration::from_millis(500), &mut runner, &mut host)
        .unwrap();
    let onset = assert_due_presentation(&beat, &runtime);
    assert_eq!(onset["transportFlash"], "beat");
    assert_eq!(onset["eventDotOn"], true);
    assert_eq!(onset["display"]["title"], original_title);
    assert_eq!(onset["leds"]["width"], platform_core::GRID_WIDTH);
    assert_eq!(onset["leds"]["height"], platform_core::GRID_HEIGHT);
    assert_eq!(
        onset["leds"]["rgb"].as_array().unwrap().len(),
        platform_core::GRID_WIDTH * platform_core::GRID_HEIGHT * 3
    );
    let onset_revision = runtime.oled_frame_revision();
    let onset_pixels = runtime.last_oled_frame().unwrap().to_vec();

    runner.test_set_display_time(start + Duration::from_millis(300));
    let expired = runtime
        .dispatch_host_message(
            HostMessage::TransportPulseStep {
                pulses: 0,
                source: SyncSource::Internal,
                at_ppqn_pulse: runtime
                    .last_status()
                    .map(|status| status.current_ppqn_pulse),
                request_snapshot: Some(false),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    let expiry = assert_due_presentation(&expired, &runtime);
    assert_eq!(expiry["eventDotOn"], false);
    assert_eq!(expiry["transportFlash"], "none");
    assert_eq!(expiry["display"]["title"], original_title);
    assert!(runtime.oled_frame_revision() > onset_revision);
    assert_ne!(runtime.last_oled_frame(), Some(onset_pixels.as_slice()));
}

#[test]
fn late_default_patch_batch_is_explicit_and_normal_cadence_resumes() {
    let (mut runtime, mut runner, mut host, start) = default_playback();
    runner.test_set_display_time(start + Duration::from_millis(91));
    let mut batches = Vec::new();
    let mut presentations = Vec::new();
    let mut previous_pulse = runtime.last_status().unwrap().current_ppqn_pulse;
    for (index, ms) in [8, 8, 8, 80, 8, 8, 8, 8].into_iter().enumerate() {
        let before_notes = host.musical_events.len();
        let before_audio = host.audio_commands.len();
        let output = runtime
            .advance_duration_with_output(Duration::from_millis(ms), &mut runner, &mut host)
            .unwrap();
        let pulse = runtime.last_status().unwrap().current_ppqn_pulse;
        let delivered = host.musical_events[before_notes..].to_vec();
        let commands = host.audio_commands.len() - before_audio;
        assert!(output.messages.iter().all(|message| match message {
            RunnerMessage::Snapshot { snapshot } => snapshot["oledFrameRevision"]
                .as_u64()
                .is_some_and(|revision| revision > 0),
            RunnerMessage::OledFrame { revision, .. } =>
                *revision == runtime.oled_frame_revision()
                    && snapshot(&output)
                        .is_some_and(|shown| shown["oledFrameRevision"] == *revision),
            _ => true,
        }));
        presentations.push((
            output
                .messages
                .iter()
                .filter(|message| matches!(message, RunnerMessage::Snapshot { .. }))
                .count(),
            output
                .messages
                .iter()
                .filter(|message| matches!(message, RunnerMessage::OledFrame { .. }))
                .count(),
            output
                .messages
                .iter()
                .filter(|message| matches!(message, RunnerMessage::RuntimeStatus { .. }))
                .count(),
            runtime.oled_frame_revision(),
        ));
        batches.push((pulse - previous_pulse, delivered, commands));
        previous_pulse = pulse;
        if index == 3 {
            assert_eq!(
                batches[index].0, 3,
                "80ms wake explicitly batches three due pulses"
            );
        }
    }
    assert_eq!(batches[0].0, 0);
    assert_eq!(batches[1].0, 0);
    assert_eq!(batches[2].0, 1);
    assert_eq!(
        batches[4].0, 1,
        "next normal 8ms wake must resume due pulse delivery"
    );
    assert_eq!(
        presentations[3].2, 1,
        "late batch must publish a runtime status"
    );
    assert!(presentations
        .iter()
        .all(|(snapshots, frames, statuses, revision)| *snapshots <= 1
            && *frames <= 1
            && *statuses <= 1
            && *revision > 0));
    assert!(
        batches
            .iter()
            .flat_map(|(_, notes, _)| notes)
            .any(|event| matches!(event, MusicalEvent::NoteOn { .. })),
        "default patch must deliver notes through FakeHost"
    );
    assert_eq!(
        batches.iter().map(|(pulses, _, _)| pulses).sum::<u64>(),
        previous_pulse
    );
    let (mut regular, mut regular_runner, mut regular_host, regular_start) = default_playback();
    regular_runner.test_set_display_time(regular_start + Duration::from_millis(91));
    for _ in 0..17 {
        regular
            .advance_duration_with_output(
                Duration::from_millis(8),
                &mut regular_runner,
                &mut regular_host,
            )
            .unwrap();
    }
    assert_eq!(
        host.musical_events, regular_host.musical_events,
        "late batch and regular wakes must preserve ordered notes"
    );
    assert_eq!(
        host.audio_commands, regular_host.audio_commands,
        "late batch and regular wakes must preserve ordered audio commands"
    );
}
