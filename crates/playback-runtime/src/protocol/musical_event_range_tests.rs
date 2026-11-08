use super::*;
use platform_core::MusicalEvent;

#[test]
fn musical_events_clamp_to_engine_ranges() {
    let cases = [
        (
            MusicalEvent::NoteOn {
                channel: 8,
                note: 128,
                velocity: 0,
                duration_ms: Some(9),
            },
            MusicalEvent::NoteOn {
                channel: 7,
                note: 127,
                velocity: 1,
                duration_ms: Some(MIN_NOTE_DURATION_MS),
            },
        ),
        (
            MusicalEvent::NoteOn {
                channel: u8::MAX,
                note: u8::MAX,
                velocity: u8::MAX,
                duration_ms: Some(u32::MAX),
            },
            MusicalEvent::NoteOn {
                channel: 7,
                note: 127,
                velocity: 127,
                duration_ms: Some(MAX_NOTE_DURATION_MS),
            },
        ),
        (
            MusicalEvent::NoteOn {
                channel: 3,
                note: 60,
                velocity: 100,
                duration_ms: None,
            },
            MusicalEvent::NoteOn {
                channel: 3,
                note: 60,
                velocity: 100,
                duration_ms: Some(MAX_NOTE_DURATION_MS),
            },
        ),
        (
            MusicalEvent::NoteOn {
                channel: 0,
                note: 0,
                velocity: 1,
                duration_ms: Some(10),
            },
            MusicalEvent::NoteOn {
                channel: 0,
                note: 0,
                velocity: 1,
                duration_ms: Some(10),
            },
        ),
        (
            MusicalEvent::NoteOff {
                channel: 9,
                note: 200,
            },
            MusicalEvent::NoteOff {
                channel: 7,
                note: 127,
            },
        ),
        (
            MusicalEvent::Cc {
                channel: 12,
                controller: 128,
                value: 255,
            },
            MusicalEvent::Cc {
                channel: 7,
                controller: 127,
                value: 127,
            },
        ),
        (
            MusicalEvent::Cc {
                channel: 2,
                controller: 74,
                value: 0,
            },
            MusicalEvent::Cc {
                channel: 2,
                controller: 74,
                value: 0,
            },
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(clamp_musical_event(&input), expected, "{input:?}");
    }
}
