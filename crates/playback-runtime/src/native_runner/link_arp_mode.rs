use serde::{Serialize, Serializer};

/// How the Link arp orders and spreads the notes it takes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArpMode {
    None,
    Direct,
    Up,
    Down,
    Bounce,
    OutsideIn,
    Rotating,
    Random,
    OctaveSpread,
    ChordStrike,
    Strum,
}

impl ArpMode {
    pub(crate) const ALL: [Self; 11] = [
        Self::None,
        Self::Direct,
        Self::Up,
        Self::Down,
        Self::Bounce,
        Self::OutsideIn,
        Self::Rotating,
        Self::Random,
        Self::OctaveSpread,
        Self::ChordStrike,
        Self::Strum,
    ];

    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Direct => "direct",
            Self::Up => "up",
            Self::Down => "down",
            Self::Bounce => "bounce",
            Self::OutsideIn => "outside_in",
            Self::Rotating => "rotating",
            Self::Random => "random",
            Self::OctaveSpread => "octave_spread",
            Self::ChordStrike => "chord_strike",
            Self::Strum => "strum",
        }
    }

    pub(crate) fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.wire_name() == value)
    }

    /// Direct and Chord Strike sound every note at once; the other modes step through them.
    pub(super) const fn is_simultaneous(self) -> bool {
        matches!(self, Self::Direct | Self::ChordStrike)
    }
}

impl Serialize for ArpMode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}

/// Which notes the Link arp plays: the ones that trigger together, or every held note.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArpSource {
    Simultaneous,
    Held,
}

impl ArpSource {
    pub(crate) const ALL: [Self; 2] = [Self::Simultaneous, Self::Held];

    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::Simultaneous => "simultaneous",
            Self::Held => "held",
        }
    }

    pub(crate) fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|source| source.wire_name() == value)
    }
}

impl Serialize for ArpSource {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}
