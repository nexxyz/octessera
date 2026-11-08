use serde::{Serialize, Serializer};

/// Whether a Link layer steps a scan line across its grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanMode {
    None,
    Scanning,
}

impl ScanMode {
    pub(crate) const ALL: [Self; 2] = [Self::None, Self::Scanning];

    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Scanning => "scanning",
        }
    }

    pub(crate) fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.wire_name() == value)
    }
}

impl Serialize for ScanMode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}
