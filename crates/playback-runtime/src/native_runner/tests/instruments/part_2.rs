use super::*;
use crate::native_runner::modulation_sampler::{
    apply_sampler_assignments_for_instruments, apply_sampler_assignments_for_instruments_routed,
    RoutedMusicalEvents,
};

#[test]
pub(crate) fn dance_fx_payload_sanitizes_type_target_and_params() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_touch_fx_payload(&json!({
        "selected": { "fxType": "stutter", "targetKey": "bad", "params": { "rateHz": 99, "depthPct": -5, "ignored": 42 } },
        "assignments": [
            { "x": 1, "y": 2, "config": { "fxType": "pitch_shift", "targetKey": "instrument_8", "params": { "semitones": 99, "cents": -200, "mixPct": 250 } } }
        ]
    }));

    assert_eq!(runner.dance_fx_selected["targetKey"], "master");
    assert_eq!(runner.dance_fx_selected["params"]["rateHz"], 32);
    assert_eq!(runner.dance_fx_selected["params"]["depthPct"], 0);
    assert!(runner.dance_fx_selected["params"].get("ignored").is_none());
    assert_eq!(
        runner.dance_fx_assignments[0].config["params"]["semitones"],
        24
    );
    assert_eq!(
        runner.dance_fx_assignments[0].config["params"]["cents"],
        -100
    );
    assert_eq!(
        runner.dance_fx_assignments[0].config["params"]["mixPct"],
        100
    );
}

#[test]
pub(crate) fn sample_assign_mode_supports_shift_row_and_fn_shift_column() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.sample_assign = Some((0, 2));

    runner.ui.shift_held = true;
    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(runner.instruments[0].sample_assignments.len(), GRID_WIDTH);
    assert!(runner.instruments[0]
        .sample_assignments
        .iter()
        .all(|assignment| assignment.y == 3 && assignment.sample_slot == 2));

    runner.instruments[0].sample_assignments.clear();
    runner.ui.shift_held = false;
    runner.ui.combined_modifier_held = true;
    runner.handle_sample_assignment_grid_press(4, 1);
    assert_eq!(runner.instruments[0].sample_assignments.len(), GRID_HEIGHT);
    assert!(runner.instruments[0]
        .sample_assignments
        .iter()
        .all(|assignment| assignment.x == 4 && assignment.sample_slot == 2));
}

#[test]
pub(crate) fn sample_assignment_cycles_velocity_levels_when_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_velocity_levels_enabled = true;
    runner.sample_assign = Some((0, 2));

    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(
        runner.instruments[0].sample_assignments[0].level.as_deref(),
        Some("high")
    );
    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(
        runner.instruments[0].sample_assignments[0].level.as_deref(),
        Some("medium")
    );
    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(
        runner.instruments[0].sample_assignments[0].level.as_deref(),
        Some("low")
    );
    runner.handle_sample_assignment_grid_press(1, 3);
    assert!(runner.instruments[0].sample_assignments.is_empty());
}

#[test]
pub(crate) fn sample_assignment_velocity_level_uses_configured_values() {
    let mut instrument = NativeInstrumentSlot::new(0);
    instrument.sample_base_velocity = 80;
    instrument.sample_velocity_high = 127;
    instrument.sample_velocity_medium = 64;
    instrument.sample_velocity_low = 32;
    let base = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: None,
    };
    let high = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: Some("high".into()),
    };
    let medium = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: Some("medium".into()),
    };
    let low = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: Some("low".into()),
    };

    assert_eq!(sampler_assignment_velocity(127, &base, &instrument), 80);
    assert_eq!(sampler_assignment_velocity(127, &high, &instrument), 127);
    assert_eq!(sampler_assignment_velocity(127, &medium, &instrument), 64);
    assert_eq!(sampler_assignment_velocity(127, &low, &instrument), 32);
}

#[test]
pub(crate) fn sense_velocity_and_filter_lanes_modulate_mapped_events() {
    let sense = NativeSensePart {
        x_velocity: NativeValueLane {
            enabled: true,
            from: 10,
            to: 110,
            grid_offset: 0,
            curve: "linear".into(),
        },
        y_filter_cutoff: NativeValueLane {
            enabled: true,
            from: 20,
            to: 120,
            grid_offset: 0,
            curve: "linear".into(),
        },
        ..NativeSensePart::default()
    };
    let events = vec![MusicalEvent::NoteOn {
        channel: 2,
        note: 60,
        velocity: 100,
        duration_ms: Some(150),
    }];
    let intents = vec![CellTriggerIntent {
        x: 7,
        y: 7,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    }];

    let out = apply_sampler_assignments_for_instruments(events, &intents, 0, &[], Some(&sense));

    assert!(matches!(
        out.as_slice(),
        [
            MusicalEvent::Cc {
                channel: 2,
                controller: 74,
                value: 120
            },
            MusicalEvent::NoteOn { velocity: 110, .. }
        ]
    ));
}

#[test]
pub(crate) fn midi_instrument_channel_remaps_note_and_cc_events() {
    let mut instrument = NativeInstrumentSlot {
        kind: "midi".into(),
        midi_channel: 10,
        ..NativeInstrumentSlot::new(0)
    };
    instrument.midi_enabled = true;
    let sense = NativeSensePart {
        y_filter_cutoff: NativeValueLane {
            enabled: true,
            from: 20,
            to: 120,
            grid_offset: 0,
            curve: "linear".into(),
        },
        ..NativeSensePart::default()
    };
    let intent = CellTriggerIntent {
        x: 0,
        y: 7,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };

    let out = apply_sampler_assignments_for_instruments(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(120),
        }],
        &[intent],
        0,
        &[instrument],
        Some(&sense),
    );

    assert!(matches!(
        out.as_slice(),
        [
            MusicalEvent::Cc { channel: 9, .. },
            MusicalEvent::NoteOn { channel: 9, .. }
        ]
    ));
}

#[test]
pub(crate) fn midi_instrument_slot_two_routes_only_to_midi_channel_one() {
    let mut instruments = vec![NativeInstrumentSlot::new(0), NativeInstrumentSlot::new(1)];
    instruments.push(NativeInstrumentSlot {
        kind: "midi".into(),
        midi_enabled: true,
        midi_channel: 2,
        ..NativeInstrumentSlot::new(2)
    });
    let intent = CellTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };

    let routed = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 2,
            note: 64,
            velocity: 100,
            duration_ms: Some(120),
        }],
        std::slice::from_ref(&intent),
        0,
        &instruments,
        None,
        0,
        None,
    );

    assert!(routed.audio.is_empty());
    assert!(matches!(
        routed.midi.as_slice(),
        [MusicalEvent::NoteOn {
            channel: 1,
            note: 64,
            ..
        }]
    ));

    instruments[2].midi_enabled = false;
    let muted = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 2,
            note: 64,
            velocity: 100,
            duration_ms: Some(120),
        }],
        &[intent],
        0,
        &instruments,
        None,
        0,
        None,
    );
    assert!(muted.audio.is_empty());
    assert!(muted.midi.is_empty());

    instruments[2].midi_enabled = true;
    let note_off = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOff {
            channel: 2,
            note: 64,
        }],
        &[],
        0,
        &instruments,
        None,
        0,
        None,
    );
    assert!(note_off.audio.is_empty());
    assert!(matches!(
        note_off.midi.as_slice(),
        [MusicalEvent::NoteOff {
            channel: 1,
            note: 64
        }]
    ));

    instruments[2].midi_enabled = false;
    let muted_note_off = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOff {
            channel: 2,
            note: 64,
        }],
        &[],
        0,
        &instruments,
        None,
        0,
        None,
    );
    assert!(muted_note_off.audio.is_empty());
    assert!(muted_note_off.midi.is_empty());
}

#[test]
pub(crate) fn dance_transpose_offsets_note_based_routes_but_not_sampler_assignments() {
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
pub(crate) fn dance_transpose_note_off_uses_original_transposed_note() {
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
pub(crate) fn dance_transpose_tracks_only_explicit_release_notes() {
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
pub(crate) fn dance_transpose_tracks_clamped_explicit_release_notes() {
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
pub(crate) fn cross_part_duplicate_note_ons_keep_highest_velocity_per_route() {
    let mut routed = RoutedMusicalEvents {
        audio: vec![
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 50,
                duration_ms: Some(80),
            },
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
                duration_ms: Some(40),
            },
        ],
        midi: vec![
            MusicalEvent::NoteOn {
                channel: 1,
                note: 64,
                velocity: 20,
                duration_ms: Some(40),
            },
            MusicalEvent::NoteOn {
                channel: 1,
                note: 64,
                velocity: 90,
                duration_ms: Some(120),
            },
        ],
    };

    routed.dedupe_note_ons_by_highest_velocity();

    assert!(matches!(
        routed.audio.as_slice(),
        [MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(80)
        }]
    ));
    assert!(matches!(
        routed.midi.as_slice(),
        [MusicalEvent::NoteOn {
            channel: 1,
            note: 64,
            velocity: 90,
            duration_ms: Some(120)
        }]
    ));
}
