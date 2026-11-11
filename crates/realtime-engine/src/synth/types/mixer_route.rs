use super::BUS_COUNT;
use serde::{de, ser, Deserialize, Deserializer, Serialize, Serializer};

/// Where an instrument's mixer strip sends its signal; `FxBus` holds the zero-based bus index.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MixerRoute {
    #[default]
    Direct,
    FxBus(usize),
}

const FX_BUS_WIRE_NAMES: [&str; BUS_COUNT] = ["fx_bus_1", "fx_bus_2", "fx_bus_3", "fx_bus_4"];

impl MixerRoute {
    pub const ALL: [Self; BUS_COUNT + 1] = [
        Self::Direct,
        Self::FxBus(0),
        Self::FxBus(1),
        Self::FxBus(2),
        Self::FxBus(3),
    ];

    pub fn wire_name(self) -> Option<&'static str> {
        match self {
            Self::Direct => Some("direct"),
            Self::FxBus(index) => FX_BUS_WIRE_NAMES.get(index).copied(),
        }
    }

    pub fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|route| route.wire_name() == Some(value))
    }

    pub const fn bus_index(self) -> Option<usize> {
        match self {
            Self::Direct => None,
            Self::FxBus(index) => Some(index),
        }
    }
}

impl Serialize for MixerRoute {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let name = self
            .wire_name()
            .ok_or_else(|| ser::Error::custom(format!("unknown mixer route {self:?}")))?;
        serializer.serialize_str(name)
    }
}

impl<'de> Deserialize<'de> for MixerRoute {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::from_wire_name(&value)
            .ok_or_else(|| de::Error::custom(format!("unknown mixer route `{value}`")))
    }
}
