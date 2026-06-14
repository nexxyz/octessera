use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MusicalEvent {
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
        #[serde(default, rename = "durationMs")]
        duration_ms: Option<u32>,
    },
    NoteOff {
        channel: u8,
        note: u8,
    },
    Cc {
        channel: u8,
        controller: u8,
        value: u8,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn musical_event_kinds_serialize_with_supported_tags() {
        let note_on = serde_json::to_value(MusicalEvent::NoteOn {
            channel: 1,
            note: 60,
            velocity: 90,
            duration_ms: Some(120),
        })
        .unwrap();
        assert_eq!(note_on["type"], "note_on");
        assert_eq!(note_on["durationMs"], 120);

        let note_off = serde_json::to_value(MusicalEvent::NoteOff {
            channel: 1,
            note: 60,
        })
        .unwrap();
        assert_eq!(note_off["type"], "note_off");

        let cc = serde_json::to_value(MusicalEvent::Cc {
            channel: 1,
            controller: 74,
            value: 100,
        })
        .unwrap();
        assert_eq!(cc["type"], "cc");
    }
}
