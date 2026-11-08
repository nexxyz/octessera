use platform_core::MusicalEvent;
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;

pub const MIN_NOTE_DURATION_MS: u32 = 10;
pub const MAX_NOTE_DURATION_MS: u32 = 86_400_000;

pub fn clamp_musical_event(event: &MusicalEvent) -> MusicalEvent {
    let channel = |channel: u8| channel.min((INSTRUMENT_SLOT_COUNT - 1) as u8);
    match *event {
        MusicalEvent::NoteOn {
            channel: slot,
            note,
            velocity,
            duration_ms,
        } => MusicalEvent::NoteOn {
            channel: channel(slot),
            note: note.min(127),
            velocity: velocity.clamp(1, 127),
            duration_ms: Some(
                duration_ms
                    .unwrap_or(MAX_NOTE_DURATION_MS)
                    .clamp(MIN_NOTE_DURATION_MS, MAX_NOTE_DURATION_MS),
            ),
        },
        MusicalEvent::NoteOff {
            channel: slot,
            note,
        } => MusicalEvent::NoteOff {
            channel: channel(slot),
            note: note.min(127),
        },
        MusicalEvent::Cc {
            channel: slot,
            controller,
            value,
        } => MusicalEvent::Cc {
            channel: channel(slot),
            controller: controller.min(127),
            value: value.min(127),
        },
    }
}
