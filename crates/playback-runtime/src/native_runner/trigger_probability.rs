use super::{NativeLinkLayer, GRID_HEIGHT, GRID_WIDTH};
use platform_core::TileTriggerIntent;
use serde::{Serialize, Serializer};

/// A layer's trigger probability: every trigger off, the tile map, or every trigger on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TriggerProbabilityMode {
    Zero,
    Custom,
    Full,
}

impl TriggerProbabilityMode {
    pub(crate) const ALL: [Self; 3] = [Self::Zero, Self::Custom, Self::Full];

    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::Custom => "custom",
            Self::Full => "full",
        }
    }

    pub(crate) fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.wire_name() == value)
    }
}

impl Serialize for TriggerProbabilityMode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}

/// One tile of a layer's custom trigger probability map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TileProbability {
    Zero,
    Low,
    High,
    Full,
}

impl TileProbability {
    pub(crate) const ALL: [Self; 4] = [Self::Zero, Self::Low, Self::High, Self::Full];

    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::Low => "low",
            Self::High => "high",
            Self::Full => "full",
        }
    }

    pub(crate) fn from_wire_name(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tile| tile.wire_name() == value)
    }

    /// The next state a probability stamp paints: zero, low, high, full, then zero again.
    pub(super) const fn next(self) -> Self {
        match self {
            Self::Zero => Self::Low,
            Self::Low => Self::High,
            Self::High => Self::Full,
            Self::Full => Self::Zero,
        }
    }
}

impl Serialize for TileProbability {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}

pub(super) type TileProbabilityMap = [TileProbability; GRID_WIDTH * GRID_HEIGHT];

pub(super) const FULL_PROBABILITY_MAP: TileProbabilityMap =
    [TileProbability::Full; GRID_WIDTH * GRID_HEIGHT];

pub(super) fn trigger_probability_allows(
    link_layer: Option<&NativeLinkLayer>,
    map: Option<&TileProbabilityMap>,
    rng: &mut u64,
    intent: &TileTriggerIntent,
) -> bool {
    let pct = trigger_probability_pct(link_layer, map, intent.x, intent.y);
    if pct == 0 {
        return false;
    }
    if pct >= 100 {
        return true;
    }
    next_probability_random(rng) < f64::from(pct) / 100.0
}

fn trigger_probability_pct(
    link_layer: Option<&NativeLinkLayer>,
    map: Option<&TileProbabilityMap>,
    x: usize,
    y: usize,
) -> u8 {
    let Some(layer) = link_layer else {
        return 100;
    };
    match layer.trigger_probability_mode {
        TriggerProbabilityMode::Zero => 0,
        TriggerProbabilityMode::Custom => {
            let tile = map
                .and_then(|map| map.get(y.saturating_mul(GRID_WIDTH).saturating_add(x)))
                .copied()
                .unwrap_or(TileProbability::Full);
            match tile {
                TileProbability::Zero => 0,
                TileProbability::Low => layer
                    .trigger_probability_low_pct
                    .min(layer.trigger_probability_high_pct),
                TileProbability::High => layer
                    .trigger_probability_high_pct
                    .max(layer.trigger_probability_low_pct),
                TileProbability::Full => 100,
            }
        }
        TriggerProbabilityMode::Full => 100,
    }
}

fn next_probability_random(rng: &mut u64) -> f64 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*rng >> 11) as f64) / ((1_u64 << 53) as f64)
}
