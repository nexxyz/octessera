use super::*;
use crate::native_runner::modulation_sampler::apply_sampler_assignments_for_instruments_routed;

#[test]
pub(crate) fn sparks_transpose_offsets_note_based_routes_but_not_sampler_assignments() {
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

    let sampler = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(120),
        }],
        &[intent],
        0,
        &[NativeInstrumentSlot {
            kind: "sampler".into(),
            sample_assignments: vec![NativeSampleAssignment {
                x: 2,
                y: 3,
                sample_slot: 4,
                level: None,
            }],
            ..NativeInstrumentSlot::new(0)
        }],
        None,
        7,
        None,
    );
    assert!(matches!(
        sampler.audio.as_slice(),
        [MusicalEvent::NoteOn { note: 40, .. }]
    ));
}

#[test]
pub(crate) fn sparks_transpose_note_off_uses_original_transposed_note() {
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
pub(crate) fn sparks_transpose_tracks_only_explicit_release_notes() {
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
pub(crate) fn sparks_transpose_tracks_clamped_explicit_release_notes() {
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
