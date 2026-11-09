use super::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    TileHit,
};
use platform_core::MusicalEvent;
use realtime_engine::synth::{INSTRUMENT_SLOT_COUNT, TILE_PITCH_CENTS_LIMIT};

pub(crate) const MIN_NOTE_DURATION_MS: u32 = 10;
pub(crate) const MAX_NOTE_DURATION_MS: u32 = 86_400_000;

pub fn clamp_note_duration_ms(duration_ms: Option<u32>) -> u32 {
    duration_ms
        .unwrap_or(MAX_NOTE_DURATION_MS)
        .clamp(MIN_NOTE_DURATION_MS, MAX_NOTE_DURATION_MS)
}

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
            duration_ms: Some(clamp_note_duration_ms(duration_ms)),
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

pub fn validate_tile_hit(hit: &TileHit) -> Result<(), RuntimeAdapterError> {
    if usize::from(hit.instrument_slot) >= INSTRUMENT_SLOT_COUNT
        || hit.voice >= 8
        || !(-TILE_PITCH_CENTS_LIMIT..=TILE_PITCH_CENTS_LIMIT).contains(&hit.pitch_cents)
        || hit.velocity > 127
    {
        return Err(RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::InvalidPayload,
            RuntimeOperation::MusicalEvent,
            Some("invalid tile hit slot, voice, pitch, or velocity".into()),
        )));
    }
    Ok(())
}
