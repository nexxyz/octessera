use super::*;
use crate::interpretation::{CellTriggerIntent, CellTriggerKind};

#[test]
fn maps_note_on_and_note_off() {
    let config = default_mapping_config();
    let result = map_intents_to_musical_events(
        &[
            CellTriggerIntent {
                x: 0,
                y: 0,
                kind: CellTriggerKind::Activate,
                degree: 0,
            },
            CellTriggerIntent {
                x: 1,
                y: 0,
                kind: CellTriggerKind::Deactivate,
                degree: 1,
            },
            CellTriggerIntent {
                x: 2,
                y: 0,
                kind: CellTriggerKind::Stable,
                degree: 2,
            },
        ],
        &config,
    );

    assert_eq!(
        result.events,
        vec![
            MusicalEvent::NoteOn {
                channel: 0,
                note: 24,
                velocity: 96,
                duration_ms: Some(150)
            },
            MusicalEvent::NoteOff {
                channel: 0,
                note: 27
            },
        ]
    );
    assert_eq!(result.intents.len(), 1);
}

#[test]
fn wraps_notes_into_range() {
    let mut config = default_mapping_config();
    config.base_midi_note = 60;
    config.max_midi_note = 64;
    let result = map_intents_to_musical_events(
        &[CellTriggerIntent {
            x: 0,
            y: 0,
            kind: CellTriggerKind::Activate,
            degree: 9,
        }],
        &config,
    );
    assert_eq!(
        result.events,
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 63,
            velocity: 96,
            duration_ms: Some(150)
        }]
    );
}

#[test]
fn wrapped_notes_stay_on_scale_when_starting_note_is_off_scale() {
    let mut config = default_mapping_config();
    config.base_midi_note = 61;
    config.max_midi_note = 72;
    config.scale = vec![0, 2, 4, 5, 7, 9, 11];
    config.range_mode = RangeMode::Wrap;

    let result = map_intents_to_musical_events(
        &[CellTriggerIntent {
            x: 0,
            y: 0,
            kind: CellTriggerKind::Activate,
            degree: 64,
        }],
        &config,
    );

    let MusicalEvent::NoteOn { note, .. } = result.events[0] else {
        panic!("expected note on");
    };
    assert!(note_matches_scale(note, &validate_config(&config)));
    assert_ne!(note, 61);
}

#[test]
fn negative_wrapped_notes_stay_on_selected_scale() {
    let mut config = default_mapping_config();
    config.base_midi_note = 60;
    config.starting_midi_note = 64;
    config.max_midi_note = 72;
    config.scale = vec![0, 2, 4, 7, 9];
    config.range_mode = RangeMode::Wrap;

    let result = map_intents_to_musical_events(
        &[CellTriggerIntent {
            x: 0,
            y: 0,
            kind: CellTriggerKind::Activate,
            degree: -3,
        }],
        &config,
    );

    let MusicalEvent::NoteOn { note, .. } = result.events[0] else {
        panic!("expected note on");
    };
    assert!(note_matches_scale(note, &validate_config(&config)));
}

#[test]
fn supports_none_note_off_and_scanned_empty_targets() {
    let mut config = default_mapping_config();
    config.scanned_empty = TriggerTarget {
        action: TriggerAction::NoteOn,
        channel: 3,
        velocity: 55,
        duration_ms: 70,
    };
    config.scanned.action = TriggerAction::None;
    let result = map_intents_to_musical_events(
        &[
            CellTriggerIntent {
                x: 0,
                y: 0,
                kind: CellTriggerKind::Scanned,
                degree: 0,
            },
            CellTriggerIntent {
                x: 1,
                y: 0,
                kind: CellTriggerKind::ScannedEmpty,
                degree: 1,
            },
            CellTriggerIntent {
                x: 2,
                y: 0,
                kind: CellTriggerKind::Deactivate,
                degree: 2,
            },
        ],
        &config,
    );
    assert_eq!(result.events.len(), 2);
    assert_eq!(
        result.events[0],
        MusicalEvent::NoteOn {
            channel: 3,
            note: 27,
            velocity: 55,
            duration_ms: Some(70)
        }
    );
    assert!(matches!(
        result.events[1],
        MusicalEvent::NoteOff { note: 29, .. }
    ));
}

#[test]
fn mapping_config_sanitizes_out_of_range_values_and_empty_scale() {
    let mut config = default_mapping_config();
    config.base_midi_note = -10;
    config.max_midi_note = 200;
    config.scale = vec![];
    config.row_step_degrees = -5;
    config.column_step_degrees = -2;
    config.activate.channel = 99;
    config.activate.velocity = 250;
    config.activate.duration_ms = 0;

    let result = map_intents_to_musical_events(
        &[CellTriggerIntent {
            x: 0,
            y: 0,
            kind: CellTriggerKind::Activate,
            degree: 0,
        }],
        &config,
    );

    assert_eq!(
        result.events,
        vec![MusicalEvent::NoteOn {
            channel: 15,
            note: 24,
            velocity: 127,
            duration_ms: Some(1)
        }]
    );
}

#[test]
fn range_mode_clamp_and_wrap_differ_for_high_degree() {
    let mut config = default_mapping_config();
    config.base_midi_note = 60;
    config.max_midi_note = 64;
    config.scale = vec![0, 2, 4, 5, 7];
    config.range_mode = RangeMode::Clamp;
    let clamped = map_intents_to_musical_events(
        &[CellTriggerIntent {
            x: 0,
            y: 0,
            kind: CellTriggerKind::Activate,
            degree: 9,
        }],
        &config,
    );
    config.range_mode = RangeMode::Wrap;
    let wrapped = map_intents_to_musical_events(
        &[CellTriggerIntent {
            x: 0,
            y: 0,
            kind: CellTriggerKind::Activate,
            degree: 9,
        }],
        &config,
    );

    assert_ne!(clamped.events, wrapped.events);
    assert_eq!(
        clamped.events[0],
        MusicalEvent::NoteOn {
            channel: 0,
            note: 64,
            velocity: 96,
            duration_ms: Some(150)
        }
    );
}

#[test]
fn life_sized_degree_sweep_stays_inside_mapping_range_and_scale() {
    let mut config = default_mapping_config();
    config.base_midi_note = 50;
    config.starting_midi_note = 57;
    config.max_midi_note = 74;
    config.scale = vec![1, 3, 6, 8, 10];
    config.range_mode = RangeMode::Clamp;

    let intents = (-128..=128)
        .map(|degree| CellTriggerIntent {
            x: 0,
            y: 0,
            kind: CellTriggerKind::Activate,
            degree,
        })
        .collect::<Vec<_>>();

    let result = map_intents_to_musical_events(&intents, &config);

    for event in result.events {
        let MusicalEvent::NoteOn { note, .. } = event else {
            panic!("expected note on");
        };
        assert!((50..=74).contains(&i32::from(note)));
        assert!(note_matches_scale(note, &validate_config(&config)));
    }
}
