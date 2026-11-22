use super::*;
use crate::native_runner::modulation::sampler::apply_sampler_assignments_for_instruments_routed;

#[test]
pub(crate) fn play_transpose_offsets_note_based_routes_but_not_default_sampler_tiles() {
    let intent = TileTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::TileTriggerKind::Activate,
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
            kind: InstrumentType::Midi,
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
                kind: InstrumentType::Sampler,
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
        fixed.tiles.as_slice(),
        [crate::TileHit {
            voice: 4,
            tile: 26,
            pitch_cents: 170,
            ..
        }]
    ));
    assert_eq!(sampler_hit(true, false, 72).tiles[0].pitch_cents, 870);
    assert_eq!(sampler_hit(false, true, 72).tiles[0].pitch_cents, 1370);
    assert_eq!(sampler_hit(false, true, 55).tiles[0].pitch_cents, -330);
    assert_eq!(sampler_hit(true, true, 72).tiles[0].pitch_cents, 2070);
    assert_eq!(sampler_hit(false, true, 127).tiles[0].pitch_cents, 4800);
}

#[test]
pub(crate) fn play_transpose_note_off_uses_original_transposed_note() {
    let intent = TileTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::TileTriggerKind::Activate,
    };
    let instruments = [NativeInstrumentSlot::new(0)];
    let mut active_notes = crate::native_runner::held_notes::HeldNotes::default();
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
    let intent = TileTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::TileTriggerKind::Activate,
    };
    let instruments = [NativeInstrumentSlot::new(0)];
    let mut active_notes = crate::native_runner::held_notes::HeldNotes::default();

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
    let intent = TileTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::TileTriggerKind::Activate,
    };
    let instruments = [NativeInstrumentSlot {
        kind: InstrumentType::Sampler,
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
    let mut active_notes = crate::native_runner::held_notes::HeldNotes::default();

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
        note_on.tiles.as_slice(),
        [crate::TileHit {
            voice: 4,
            tile: 26,
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
        note_off.tiles.as_slice(),
        [crate::TileHit {
            instrument_slot: 0,
            voice: 4,
            tile: 26,
            velocity: 0,
            ..
        }]
    ));
    assert!(active_notes.is_empty());
}

#[test]
pub(crate) fn play_transpose_tracks_clamped_explicit_release_notes() {
    let intent = TileTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::TileTriggerKind::Activate,
    };
    let instruments = [NativeInstrumentSlot::new(0)];
    let mut active_notes = crate::native_runner::held_notes::HeldNotes::default();
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
