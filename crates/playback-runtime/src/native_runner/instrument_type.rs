use serde::{Serialize, Serializer};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InstrumentType {
    None,
    Synth,
    Fm,
    Pluck,
    Drum,
    Sampler,
    Midi,
    MidiDrum,
}

impl InstrumentType {
    pub const ALL: [Self; 8] = [
        Self::None,
        Self::Synth,
        Self::Fm,
        Self::Pluck,
        Self::Drum,
        Self::Sampler,
        Self::Midi,
        Self::MidiDrum,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Synth => "synth",
            Self::Fm => "fm",
            Self::Pluck => "pluck",
            Self::Drum => "drum",
            Self::Sampler => "sampler",
            Self::Midi => "midi",
            Self::MidiDrum => "midi_drum",
        }
    }

    pub fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.wire_name() == value)
    }

    pub const fn is_midi(self) -> bool {
        matches!(self, Self::Midi | Self::MidiDrum)
    }
}

impl Serialize for InstrumentType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}
