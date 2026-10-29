use platform_core::MusicalEvent;
use std::collections::BTreeMap;
use std::time::Duration;

/// Notes started on each output lane, keyed by `(audio, channel, note)`, with the runtime time
/// they stop sounding on their own; held notes never do.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SoundingNotes {
    until: BTreeMap<(bool, u8, u8), Duration>,
}

impl SoundingNotes {
    /// Records note-ons and returns false for a note-off whose note is not sounding.
    pub(super) fn admit(&mut self, audio: bool, event: &MusicalEvent, now: Duration) -> bool {
        match *event {
            MusicalEvent::NoteOn {
                channel,
                note,
                duration_ms,
                ..
            } => {
                let until = duration_ms.map_or(Duration::MAX, |duration_ms| {
                    now.saturating_add(Duration::from_millis(u64::from(duration_ms)))
                });
                let sounding = self.until.entry((audio, channel, note)).or_default();
                *sounding = (*sounding).max(until);
                true
            }
            MusicalEvent::NoteOff { channel, note } => self
                .until
                .remove(&(audio, channel, note))
                .is_some_and(|until| until > now),
            MusicalEvent::Cc { .. } => true,
        }
    }

    pub(super) fn clear(&mut self) {
        self.until.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn note_on(duration_ms: Option<u32>) -> MusicalEvent {
        MusicalEvent::NoteOn {
            channel: 1,
            note: 60,
            velocity: 100,
            duration_ms,
        }
    }

    const NOTE_OFF: MusicalEvent = MusicalEvent::NoteOff {
        channel: 1,
        note: 60,
    };

    #[test]
    fn note_off_passes_only_while_its_note_sounds() {
        let mut notes = SoundingNotes::default();
        assert!(!notes.admit(true, &NOTE_OFF, Duration::ZERO));
        assert!(notes.admit(true, &note_on(Some(100)), Duration::ZERO));
        assert!(!notes.admit(false, &NOTE_OFF, MS * 50));
        assert!(notes.admit(true, &NOTE_OFF, MS * 50));
        assert!(!notes.admit(true, &NOTE_OFF, MS * 60));
        assert!(notes.admit(true, &note_on(Some(100)), MS * 100));
        assert!(!notes.admit(true, &NOTE_OFF, MS * 200));
    }

    #[test]
    fn retrigger_keeps_the_longest_sounding_note() {
        let mut notes = SoundingNotes::default();
        notes.admit(true, &note_on(Some(100)), Duration::ZERO);
        notes.admit(true, &note_on(Some(100)), MS * 80);
        assert!(notes.admit(true, &NOTE_OFF, MS * 150));
        notes.admit(true, &note_on(None), MS * 200);
        notes.admit(true, &note_on(Some(10)), MS * 210);
        assert!(notes.admit(true, &NOTE_OFF, MS * 10_000));
    }

    #[test]
    fn clear_forgets_silenced_notes() {
        let mut notes = SoundingNotes::default();
        notes.admit(true, &note_on(None), Duration::ZERO);
        notes.clear();
        assert!(!notes.admit(true, &NOTE_OFF, MS * 1));
    }
}
