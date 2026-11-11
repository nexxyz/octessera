use super::in_place_menu_actions::confirm_key;
use super::*;
use crate::timing_units::BAR_PULSES;

fn delayed_release_runner(midi: bool) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let mut instrument = NativeInstrumentSlot::new(0);
    instrument.note_behavior = NoteBehavior::Hold;
    if midi {
        instrument.kind = InstrumentType::Midi;
        instrument.midi_enabled = true;
        instrument.midi_channel = 3;
        runner.midi.enabled = true;
    }
    runner.instruments[0] = instrument;
    runner.layers[0].link.deactivate_timing.delay_steps = 2;
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.sync_engine_runtime_config();
    runner
}

fn note_events(messages: &[RunnerMessage], midi: bool, note_on: bool) -> Vec<(u8, u8)> {
    routed_events(messages, midi)
        .iter()
        .filter_map(|event| match event {
            MusicalEvent::NoteOn { channel, note, .. } if note_on => Some((*channel, *note)),
            MusicalEvent::NoteOff { channel, note } if !note_on => Some((*channel, *note)),
            _ => None,
        })
        .collect()
}

/// Presses and releases a held key so its note-off waits in the Link delay queue.
fn sound_and_queue_release(runner: &mut NativeRunner, midi: bool) -> (u8, u8) {
    let pressed = device_input(runner, json!({ "type": "grid_press", "x": 2, "y": 3 }));
    let notes = note_events(&pressed, midi, true);
    assert_eq!(notes.len(), 1, "midi {midi}");
    let released = device_input(runner, json!({ "type": "grid_release", "x": 2, "y": 3 }));
    assert!(
        note_events(&released, midi, false).is_empty(),
        "midi {midi}"
    );
    assert!(!runner.delayed_link_events[0].is_empty(), "midi {midi}");
    notes[0]
}

fn assert_path_releases_the_queued_note(
    path: &str,
    prepare: impl Fn(&mut NativeRunner),
    run: impl Fn(&mut NativeRunner) -> Vec<RunnerMessage>,
) {
    for midi in [false, true] {
        let mut runner = delayed_release_runner(midi);
        prepare(&mut runner);
        let note = sound_and_queue_release(&mut runner, midi);
        let mut messages = run(&mut runner);
        messages.extend(runner.messages_with_snapshot().unwrap());
        assert_eq!(
            note_events(&messages, midi, false),
            vec![note],
            "{path}, midi {midi}"
        );
        assert!(
            runner.layers[0].emitted_note_owners.is_empty(),
            "{path}, midi {midi}"
        );
    }
}

#[test]
pub(crate) fn behavior_change_releases_a_note_whose_note_off_was_delayed() {
    assert_path_releases_the_queued_note(
        "behavior change",
        |_| {},
        |runner| {
            runner.select_behavior("life").unwrap();
            Vec::new()
        },
    );
}

#[test]
pub(crate) fn behavior_reset_releases_a_note_whose_note_off_was_delayed() {
    assert_path_releases_the_queued_note(
        "reset",
        |_| {},
        |runner| {
            runner
                .execute_menu_action(crate::native_menu::NativeMenuAction::ResetBehavior)
                .unwrap();
            Vec::new()
        },
    );
}

#[test]
pub(crate) fn external_resync_releases_a_note_whose_note_off_was_delayed() {
    assert_path_releases_the_queued_note(
        "external resync",
        |runner| {
            runner.midi.enabled = true;
            runner.midi.clock_in_enabled = true;
            runner.transport.sync_source = SyncSource::External;
            runner.transport.current_ppqn_pulse = BAR_PULSES - 1;
            runner.transport.pending_resync = true;
        },
        |runner| {
            let messages = runner
                .send(HostMessage::MidiRealtimeClock { pulses: 1 })
                .unwrap();
            assert!(!runner.transport.pending_resync);
            messages
        },
    );
}

#[test]
pub(crate) fn clear_all_releases_a_note_whose_note_off_was_delayed() {
    assert_path_releases_the_queued_note(
        "clear all",
        |_| {},
        |runner| confirm_key(runner, "system.clearAll"),
    );
}
