use super::*;
use crate::native_runner::modulation_sampler::{
    apply_sampler_assignments_for_instruments_routed, TransposedHeldNote,
};

#[test]
pub(crate) fn play_transpose_offsets_note_based_routes_but_not_default_sampler_cells() {
    let intent = CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let synth = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(120),
        }],
        std::slice::from_ref(&intent),
        0,
        &[NativeInstrumentSlot::new(0)],
        None,
        7,
        None,
    );
    assert!(matches!(
        synth.audio.as_slice(),
        [MusicalEvent::NoteOn { note: 67, .. }]
    ));

    let midi = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(120),
        }],
        std::slice::from_ref(&intent),
        0,
        &[NativeInstrumentSlot {
            kind: "midi".into(),
            midi_enabled: true,
            midi_channel: 3,
            ..NativeInstrumentSlot::new(0)
        }],
        None,
        7,
        None,
    );
    assert!(matches!(
        midi.midi.as_slice(),
        [MusicalEvent::NoteOn {
            channel: 2,
            note: 67,
            ..
        }]
    ));

    let sampler_hit = |follow_transpose: bool, follow_pitch: bool, note: u8| {
        apply_sampler_assignments_for_instruments_routed(
            vec![MusicalEvent::NoteOn {
                channel: 0,
                note,
                velocity: 100,
                duration_ms: Some(120),
            }],
            std::slice::from_ref(&intent),
            0,
            &[NativeInstrumentSlot {
                kind: "sampler".into(),
                sample_follow_transpose: follow_transpose,
                sample_follow_pitch: follow_pitch,
                sample_assignments: vec![NativeSampleAssignment {
                    x: 2,
                    y: 3,
                    sample_slot: 4,
                    level: None,
                    tune_semis: 2,
                    tune_cents: -30,
                }],
                ..NativeInstrumentSlot::new(0)
            }],
            None,
            7,
            None,
        )
    };
    let fixed = sampler_hit(false, false, 72);
    assert!(fixed.audio.is_empty());
    assert!(matches!(
        fixed.cells.as_slice(),
        [crate::CellHit {
            voice: 4,
            cell: 26,
            pitch_cents: 170,
            ..
        }]
    ));
    assert_eq!(sampler_hit(true, false, 72).cells[0].pitch_cents, 870);
    assert_eq!(sampler_hit(false, true, 72).cells[0].pitch_cents, 1370);
    assert_eq!(sampler_hit(false, true, 55).cells[0].pitch_cents, -330);
    assert_eq!(sampler_hit(true, true, 72).cells[0].pitch_cents, 2070);
    assert_eq!(sampler_hit(false, true, 127).cells[0].pitch_cents, 4800);
}

#[test]
pub(crate) fn play_transpose_note_off_uses_original_transposed_note() {
    let intent = CellTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let instruments = [NativeInstrumentSlot::new(0)];
    let mut active_notes = std::collections::BTreeMap::new();
    let note_on = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: None,
        }],
        std::slice::from_ref(&intent),
        0,
        &instruments,
        None,
        7,
        Some(&mut active_notes),
    );
    assert!(matches!(
        note_on.audio.as_slice(),
        [MusicalEvent::NoteOn { note: 67, .. }]
    ));

    let note_off = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }],
        std::slice::from_ref(&intent),
        0,
        &instruments,
        None,
        0,
        Some(&mut active_notes),
    );
    assert!(matches!(
        note_off.audio.as_slice(),
        [MusicalEvent::NoteOff { note: 67, .. }]
    ));
    assert!(active_notes.is_empty());
}

#[test]
pub(crate) fn play_transpose_tracks_only_explicit_release_notes() {
    let intent = CellTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let instruments = [NativeInstrumentSlot::new(0)];
    let mut active_notes = std::collections::BTreeMap::new();

    let _ = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(120),
        }],
        std::slice::from_ref(&intent),
        0,
        &instruments,
        None,
        7,
        Some(&mut active_notes),
    );
    assert!(active_notes.is_empty());

    let _ = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: None,
        }],
        std::slice::from_ref(&intent),
        0,
        &instruments,
        None,
        0,
        Some(&mut active_notes),
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
        7,
        Some(&mut active_notes),
    );
    assert!(matches!(
        note_off.audio.as_slice(),
        [MusicalEvent::NoteOff { note: 60, .. }]
    ));
}

#[test]
pub(crate) fn sampler_hold_release_uses_the_final_routed_note_without_intent() {
    let intent = CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let instruments = [NativeInstrumentSlot {
        kind: "sampler".into(),
        sample_assignments: vec![NativeSampleAssignment {
            x: 2,
            y: 3,
            sample_slot: 4,
            level: None,
            tune_semis: 0,
            tune_cents: 0,
        }],
        ..NativeInstrumentSlot::new(0)
    }];
    let mut active_notes = std::collections::BTreeMap::new();

    let note_on = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: None,
        }],
        std::slice::from_ref(&intent),
        0,
        &instruments,
        None,
        0,
        Some(&mut active_notes),
    );
    assert!(matches!(
        note_on.cells.as_slice(),
        [crate::CellHit {
            voice: 4,
            cell: 26,
            velocity: 1..,
            ..
        }]
    ));

    let note_off = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }],
        &[],
        0,
        &instruments,
        None,
        0,
        Some(&mut active_notes),
    );
    assert!(note_off.audio.is_empty());
    assert!(matches!(
        note_off.cells.as_slice(),
        [crate::CellHit {
            instrument_slot: 0,
            voice: 4,
            cell: 26,
            velocity: 0,
            ..
        }]
    ));
    assert!(active_notes.is_empty());
}

#[test]
pub(crate) fn play_transpose_tracks_clamped_explicit_release_notes() {
    let intent = CellTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let instruments = [NativeInstrumentSlot::new(0)];
    let mut active_notes = std::collections::BTreeMap::new();
    let _ = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 127,
            velocity: 100,
            duration_ms: None,
        }],
        std::slice::from_ref(&intent),
        0,
        &instruments,
        None,
        7,
        Some(&mut active_notes),
    );
    let note_off = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 127,
        }],
        std::slice::from_ref(&intent),
        0,
        &instruments,
        None,
        -12,
        Some(&mut active_notes),
    );
    assert!(matches!(
        note_off.audio.as_slice(),
        [MusicalEvent::NoteOff { note: 127, .. }]
    ));
    assert!(active_notes.is_empty());
}

#[test]
pub(crate) fn play_transpose_retarget_drains_held_note_before_new_offset() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "transpose".into();
    runner.play_transpose_offsets[0] = 7;
    runner.play_transpose_active_notes[0]
        .entry((0, 60))
        .or_default()
        .push(TransposedHeldNote {
            routed_channel: 0,
            routed_note: 67,
            routed_to_midi: false,
            sampler_cell: None,
        });

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
    assert!(runner.play_transpose_active_notes[0].is_empty());
    assert_eq!(runner.play_transpose_offsets[0], 12);
}

#[test]
pub(crate) fn play_transpose_disable_drains_midi_held_note_to_routed_channel() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_play_mode = "transpose".into();
    runner.instruments[0].kind = "midi".into();
    runner.instruments[0].midi_enabled = true;
    runner.instruments[0].midi_channel = 3;
    runner.play_transpose_active_notes[0]
        .entry((0, 60))
        .or_default()
        .push(TransposedHeldNote {
            routed_channel: 2,
            routed_note: 67,
            routed_to_midi: true,
            sampler_cell: None,
        });

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
    assert!(runner.play_transpose_active_notes[0].is_empty());
}

#[test]
pub(crate) fn play_transpose_release_uses_stored_route_after_midi_is_disabled() {
    let intent = CellTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let instruments = [NativeInstrumentSlot {
        kind: "midi".into(),
        midi_enabled: false,
        midi_channel: 9,
        ..NativeInstrumentSlot::new(0)
    }];
    let mut active_notes = std::collections::BTreeMap::new();
    active_notes.insert(
        (0, 60),
        vec![TransposedHeldNote {
            routed_channel: 2,
            routed_note: 67,
            routed_to_midi: true,
            sampler_cell: None,
        }],
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
    runner.play_transpose_active_notes[0].insert(
        (0, 60),
        vec![TransposedHeldNote {
            routed_channel: 2,
            routed_note: 67,
            routed_to_midi: true,
            sampler_cell: None,
        }],
    );
    runner.play_transpose_active_notes[1].insert(
        (0, 64),
        vec![TransposedHeldNote {
            routed_channel: 2,
            routed_note: 71,
            routed_to_midi: true,
            sampler_cell: None,
        }],
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
    assert!(runner.play_transpose_active_notes[0].is_empty());
    assert!(runner.play_transpose_active_notes[1].is_empty());
}

#[test]
pub(crate) fn clear_patch_state_preserves_pending_transpose_note_offs() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.play_transpose_active_notes[0].insert(
        (0, 60),
        vec![TransposedHeldNote {
            routed_channel: 0,
            routed_note: 67,
            routed_to_midi: false,
            sampler_cell: None,
        }],
    );

    runner.clear_patch_state().unwrap();
    let messages = runner.messages_without_snapshot().unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events == &vec![MusicalEvent::NoteOff { channel: 0, note: 67 }]
    )));
}
