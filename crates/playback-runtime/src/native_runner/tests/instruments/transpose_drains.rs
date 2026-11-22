use super::*;
use crate::native_runner::modulation::sampler::{
    apply_sampler_assignments_for_instruments_routed, TransposedHeldNote,
};

#[test]
pub(crate) fn play_transpose_retarget_drains_held_note_before_new_offset() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Transpose);
    runner.transpose.offsets[0] = 7;
    runner.held_notes[0].record_sounding(
        0,
        60,
        TransposedHeldNote {
            routed_channel: 0,
            routed_note: 67,
            routed_to_midi: false,
            sampler_tile: None,
        },
    );

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 5 }),
            request_snapshot: Some(true),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events == &vec![MusicalEvent::NoteOff { channel: 0, note: 67 }]
    )));
    assert!(runner.held_notes[0].is_empty());
    assert_eq!(runner.transpose.offsets[0], 12);
}

#[test]
pub(crate) fn play_transpose_disable_drains_midi_held_note_to_routed_channel() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Transpose);
    runner.instruments[0].kind = InstrumentType::Midi;
    runner.instruments[0].midi_enabled = true;
    runner.instruments[0].midi_channel = 3;
    runner.held_notes[0].record_sounding(
        0,
        60,
        TransposedHeldNote {
            routed_channel: 2,
            routed_note: 67,
            routed_to_midi: true,
            sampler_tile: None,
        },
    );

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: Some(true),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::MidiEvents { events }
            if events == &vec![MusicalEvent::NoteOff { channel: 2, note: 67 }]
    )));
    assert!(runner.held_notes[0].is_empty());
}

#[test]
pub(crate) fn play_transpose_release_uses_stored_route_after_midi_is_disabled() {
    let intent = TileTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::TileTriggerKind::Activate,
    };
    let instruments = [NativeInstrumentSlot {
        kind: InstrumentType::Midi,
        midi_enabled: false,
        midi_channel: 9,
        ..NativeInstrumentSlot::new(0)
    }];
    let mut active_notes = crate::native_runner::held_notes::HeldNotes::default();
    active_notes.record_sounding(
        0,
        60,
        TransposedHeldNote {
            routed_channel: 2,
            routed_note: 67,
            routed_to_midi: true,
            sampler_tile: None,
        },
    );

    let note_off = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }],
        std::slice::from_ref(&intent),
        0,
        &instruments,
        None,
        -12,
        Some(&mut active_notes),
    );

    assert!(note_off.audio.is_empty());
    assert_eq!(
        note_off.midi,
        vec![MusicalEvent::NoteOff {
            channel: 2,
            note: 67
        }]
    );
    assert!(active_notes.is_empty());
}

#[test]
pub(crate) fn play_transpose_instrument_route_change_drains_all_original_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.held_notes[0].record_sounding(
        0,
        60,
        TransposedHeldNote {
            routed_channel: 2,
            routed_note: 67,
            routed_to_midi: true,
            sampler_tile: None,
        },
    );
    runner.held_notes[1].record_sounding(
        0,
        64,
        TransposedHeldNote {
            routed_channel: 2,
            routed_note: 71,
            routed_to_midi: true,
            sampler_tile: None,
        },
    );

    runner.drain_play_transpose_instrument_notes(0);
    let messages = runner.messages_without_snapshot().unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::MidiEvents { events }
            if events == &vec![
                MusicalEvent::NoteOff { channel: 2, note: 67 },
                MusicalEvent::NoteOff { channel: 2, note: 71 },
            ]
    )));
    assert!(runner.held_notes[0].is_empty());
    assert!(runner.held_notes[1].is_empty());
}

#[test]
pub(crate) fn clear_patch_state_preserves_pending_transpose_note_offs() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.held_notes[0].record_sounding(
        0,
        60,
        TransposedHeldNote {
            routed_channel: 0,
            routed_note: 67,
            routed_to_midi: false,
            sampler_tile: None,
        },
    );

    runner.clear_patch_state().unwrap();
    let messages = runner.messages_without_snapshot().unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events == &vec![MusicalEvent::NoteOff { channel: 0, note: 67 }]
    )));
}
