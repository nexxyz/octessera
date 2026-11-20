//! Incoming MIDI note messages from a keyboard on the selected MIDI In.

use crate::protocol::HostMessage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiNote {
    /// 0-15.
    pub channel: u8,
    pub note: u8,
    /// 0 is a note-off, so a Note On with velocity 0 parses as one.
    pub velocity: u8,
}

impl MidiNote {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let [status, note, velocity] = *bytes else {
            return None;
        };
        if note >= 0x80 || velocity >= 0x80 {
            return None;
        }
        let velocity = match status & 0xF0 {
            0x90 => velocity,
            0x80 => 0,
            _ => return None,
        };
        Some(Self {
            channel: status & 0x0F,
            note,
            velocity,
        })
    }
}

impl From<MidiNote> for HostMessage {
    fn from(note: MidiNote) -> Self {
        Self::MidiNote {
            channel: note.channel,
            note: note.note,
            velocity: note.velocity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_on_and_off_parse_and_velocity_zero_is_an_off() {
        let note = |channel, note, velocity| {
            Some(MidiNote {
                channel,
                note,
                velocity,
            })
        };
        assert_eq!(MidiNote::parse(&[0x92, 60, 100]), note(2, 60, 100));
        assert_eq!(MidiNote::parse(&[0x92, 60, 0]), note(2, 60, 0));
        assert_eq!(MidiNote::parse(&[0x85, 64, 40]), note(5, 64, 0));
        assert_eq!(MidiNote::parse(&[0xB0, 74, 100]), None);
        assert_eq!(MidiNote::parse(&[0xA0, 60, 100]), None);
        assert_eq!(MidiNote::parse(&[0x90, 60]), None);
        assert_eq!(MidiNote::parse(&[0x90, 0x80, 100]), None);
        assert_eq!(MidiNote::parse(&[0xF8]), None);
    }
}
