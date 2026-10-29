use super::*;
use crate::tests::support::FakeHost;
use crate::{PlaybackRuntime, RuntimeConfig};
use std::collections::BTreeSet;

fn scanned_empty_note_off_runtime(
    note_behavior: &str,
) -> (PlaybackRuntime, NativeRunner, FakeHost) {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.instruments[0].note_behavior = note_behavior.into();
    runner.midi_enabled = true;
    runner.selected_midi_output_id = Some("out-1".into());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.link_layers[0].scan_mode = "scanning".into();
    runner.link_layers[0].scan_axis = "columns".into();
    runner.link_layers[0].scan_unit = "1/8".into();
    runner.link_layers[0].scanned_action = "note_on".into();
    runner.link_layers[0].scanned_empty_action = "note_off".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner.sync_engine_runtime_config();
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    press(&mut runtime, &mut runner, &mut host, 2, 3);
    (runtime, runner, host)
}

fn press(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut FakeHost,
    x: u8,
    y: u8,
) {
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": x, "y": y }),
                request_snapshot: None,
            },
            runner,
            host,
        )
        .unwrap();
}

fn play(runtime: &mut PlaybackRuntime, runner: &mut NativeRunner, host: &mut FakeHost, ms: u64) {
    for _ in 0..ms / 10 {
        runtime.advance(10, runner, host).unwrap();
    }
}

fn assert_note_offs_follow_sounding_notes(events: &[MusicalEvent]) -> usize {
    let mut sounding = BTreeSet::new();
    let mut note_offs = 0;
    for event in events {
        match event {
            MusicalEvent::NoteOn { channel, note, .. } => {
                sounding.insert((*channel, *note));
            }
            MusicalEvent::NoteOff { channel, note } => {
                assert!(
                    sounding.remove(&(*channel, *note)),
                    "note-off {channel}/{note} without a sounding note in {events:?}"
                );
                note_offs += 1;
            }
            MusicalEvent::Cc { .. } => {}
        }
    }
    note_offs
}

fn wire_count(host: &FakeHost, status: u8) -> usize {
    host.midi_messages
        .iter()
        .filter(|message| message.first().is_some_and(|byte| byte & 0xF0 == status))
        .count()
}

#[test]
pub(crate) fn scanned_empty_cells_send_no_note_offs_for_silent_notes() {
    let (mut runtime, mut runner, mut host) = scanned_empty_note_off_runtime("oneshot");

    play(&mut runtime, &mut runner, &mut host, 4_000);

    let note_ons = wire_count(&host, 0x90);
    assert!(note_ons > 0, "{:?}", host.midi_messages);
    assert_eq!(
        assert_note_offs_follow_sounding_notes(&host.musical_events),
        0
    );
    assert_eq!(wire_count(&host, 0x80), note_ons);
}

#[test]
pub(crate) fn scanned_empty_cell_still_ends_a_held_note() {
    let (mut runtime, mut runner, mut host) = scanned_empty_note_off_runtime("hold");

    play(&mut runtime, &mut runner, &mut host, 4_000);

    let note_ons = wire_count(&host, 0x90);
    assert!(note_ons > 0, "{:?}", host.midi_messages);
    assert_eq!(
        assert_note_offs_follow_sounding_notes(&host.musical_events),
        note_ons
    );
    assert_eq!(wire_count(&host, 0x80), note_ons);
}
