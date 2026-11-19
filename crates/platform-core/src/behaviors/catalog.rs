use super::NativeBehavior;

/// A Build menu section; the catalog entries with its id are its behaviors.
pub struct BehaviorCategory {
    pub id: &'static str,
    pub label: &'static str,
}

pub struct BehaviorCatalogEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub category_id: &'static str,
}

const fn entry(
    behavior: NativeBehavior,
    label: &'static str,
    category_id: &'static str,
) -> BehaviorCatalogEntry {
    BehaviorCatalogEntry {
        id: behavior.id(),
        label,
        category_id,
    }
}

pub fn behavior_categories() -> &'static [BehaviorCategory] {
    &[
        BehaviorCategory {
            id: "play",
            label: "Human",
        },
        BehaviorCategory {
            id: "rhythm",
            label: "Rhythm",
        },
        BehaviorCategory {
            id: "musical",
            label: "Musical",
        },
        BehaviorCategory {
            id: "cellular",
            label: "Cellular",
        },
        BehaviorCategory {
            id: "fields",
            label: "Fields",
        },
        BehaviorCategory {
            id: "geometry",
            label: "Geometry",
        },
        BehaviorCategory {
            id: "growth",
            label: "Growth",
        },
        BehaviorCategory {
            id: "motion",
            label: "Motion",
        },
        BehaviorCategory {
            id: "games",
            label: "Games",
        },
    ]
}

/// Every behavior, grouped by category in Build menu order.
pub fn behavior_catalog() -> &'static [BehaviorCatalogEntry] {
    BEHAVIOR_CATALOG
}

const BEHAVIOR_CATALOG: &[BehaviorCatalogEntry] = &[
    entry(NativeBehavior::Keys, "keys", "play"),
    entry(NativeBehavior::Looper, "looper", "play"),
    entry(NativeBehavior::None, "none", "play"),
    entry(NativeBehavior::Sequencer, "sequencer", "play"),
    entry(NativeBehavior::Weave, "weave", "play"),
    entry(NativeBehavior::Polyrhythm, "polyrhythm", "rhythm"),
    entry(NativeBehavior::Breaks, "breaks", "rhythm"),
    entry(NativeBehavior::Fills, "fills", "rhythm"),
    entry(NativeBehavior::Clave, "clave", "rhythm"),
    entry(NativeBehavior::Groove, "groove", "rhythm"),
    entry(NativeBehavior::Euclid, "euclid", "rhythm"),
    entry(NativeBehavior::Ostinato, "ostinato", "musical"),
    entry(NativeBehavior::Motif, "motif", "musical"),
    entry(NativeBehavior::Canon, "canon", "musical"),
    entry(NativeBehavior::Chords, "chords", "musical"),
    entry(NativeBehavior::Contour, "contour", "musical"),
    entry(NativeBehavior::Cadence, "cadence", "musical"),
    entry(NativeBehavior::Phrase, "phrase", "musical"),
    entry(NativeBehavior::Ant, "ant", "cellular"),
    entry(NativeBehavior::Brain, "brain", "cellular"),
    entry(NativeBehavior::Cyclic, "cyclic", "cellular"),
    entry(NativeBehavior::ForestFire, "forest fire", "cellular"),
    entry(NativeBehavior::Life, "life", "cellular"),
    entry(NativeBehavior::PredatorPrey, "predator prey", "cellular"),
    entry(NativeBehavior::Twinkle, "twinkle", "cellular"),
    entry(NativeBehavior::Ink, "ink", "fields"),
    entry(NativeBehavior::Ising, "ising", "fields"),
    entry(NativeBehavior::Kuramoto, "kuramoto", "fields"),
    entry(NativeBehavior::Lightning, "lightning", "fields"),
    entry(NativeBehavior::Raindrops, "raindrops", "fields"),
    entry(
        NativeBehavior::ReactionDiffusion,
        "reaction diffusion",
        "fields",
    ),
    entry(NativeBehavior::Rivers, "rivers", "fields"),
    entry(NativeBehavior::Wave, "wave", "fields"),
    entry(
        NativeBehavior::FractalExplorer,
        "fractal explorer",
        "geometry",
    ),
    entry(NativeBehavior::MazeGrowth, "maze growth", "geometry"),
    entry(NativeBehavior::Shapes, "shapes", "geometry"),
    entry(NativeBehavior::Coral, "coral", "growth"),
    entry(NativeBehavior::Cracks, "cracks", "growth"),
    entry(NativeBehavior::CrystalGrowth, "crystal growth", "growth"),
    entry(NativeBehavior::Dla, "dla", "growth"),
    entry(NativeBehavior::Physarum, "physarum", "growth"),
    entry(NativeBehavior::Vines, "vines", "growth"),
    entry(NativeBehavior::Bounce, "bounce", "motion"),
    entry(NativeBehavior::Bubbles, "bubbles", "motion"),
    entry(NativeBehavior::Gravity, "gravity", "motion"),
    entry(NativeBehavior::Boids, "boids", "motion"),
    entry(NativeBehavior::LavaLamp, "lava lamp", "motion"),
    entry(NativeBehavior::Orbit, "orbit", "motion"),
    entry(NativeBehavior::SandRipples, "sand ripples", "motion"),
    entry(NativeBehavior::Blockout, "blockout", "games"),
    entry(NativeBehavior::Pong, "pong", "games"),
];
