use serde::{Serialize, Serializer};

/// The saved Play page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlayMode {
    Mix,
    Pan,
    Route,
    Tempo,
    Lfo,
    Faders,
    Fx,
    Xy,
    Transpose,
    Rate,
    TriggerGate,
}

impl PlayMode {
    pub(crate) const ALL: [Self; 11] = [
        Self::Mix,
        Self::Pan,
        Self::Route,
        Self::Tempo,
        Self::Lfo,
        Self::Faders,
        Self::Fx,
        Self::Xy,
        Self::Transpose,
        Self::Rate,
        Self::TriggerGate,
    ];

    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::Mix => "mix",
            Self::Pan => "pan",
            Self::Route => "route",
            Self::Tempo => "tempo",
            Self::Lfo => "lfo",
            Self::Faders => "faders",
            Self::Fx => "fx",
            Self::Xy => "xy",
            Self::Transpose => "transpose",
            Self::Rate => "rate",
            Self::TriggerGate => "trigger-gate",
        }
    }

    pub(crate) fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.wire_name() == value)
    }
}

impl Serialize for PlayMode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}

/// The Play page the grid shows now; `None` while the grid plays the active layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActivePlayMode {
    None,
    Page(PlayMode),
}

impl ActivePlayMode {
    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Page(mode) => mode.wire_name(),
        }
    }
}

impl Serialize for ActivePlayMode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}
