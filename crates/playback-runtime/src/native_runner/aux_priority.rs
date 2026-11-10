use serde::{Serialize, Serializer};

/// Which wins an Aux side that has both an Auto Map target and a manual binding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum AuxPriority {
    #[default]
    Manual,
    Auto,
}

impl AuxPriority {
    pub(crate) const ALL: [Self; 2] = [Self::Manual, Self::Auto];

    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
        }
    }

    pub(crate) fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|priority| priority.wire_name() == value)
    }
}

impl Serialize for AuxPriority {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}
