pub struct BehaviorCategory {
    pub id: &'static str,
    pub label: &'static str,
    pub behavior_ids: &'static [&'static str],
}

pub struct BehaviorCatalogEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub category_id: &'static str,
}

pub fn behavior_categories() -> &'static [BehaviorCategory] {
    &[
        BehaviorCategory {
            id: "play",
            label: "Human",
            behavior_ids: &["keys", "looper", "none", "sequencer", "weave"],
        },
        BehaviorCategory {
            id: "rhythm",
            label: "Rhythm",
            behavior_ids: &["polyrhythm", "breaks", "fills", "clave", "groove", "euclid"],
        },
        BehaviorCategory {
            id: "musical",
            label: "Musical",
            behavior_ids: &[
                "ostinato", "motif", "canon", "chords", "contour", "cadence", "phrase",
            ],
        },
        BehaviorCategory {
            id: "cellular",
            label: "Cellular",
            behavior_ids: &[
                "ant",
                "brain",
                "cyclic",
                "forest_fire",
                "life",
                "predator_prey",
            ],
        },
        BehaviorCategory {
            id: "fields",
            label: "Fields",
            behavior_ids: &[
                "ink",
                "ising",
                "kuramoto",
                "lightning",
                "raindrops",
                "reaction_diffusion",
                "rivers",
                "wave",
            ],
        },
        BehaviorCategory {
            id: "geometry",
            label: "Geometry",
            behavior_ids: &["fractal_explorer", "maze_growth", "shapes"],
        },
        BehaviorCategory {
            id: "growth",
            label: "Growth",
            behavior_ids: &[
                "coral",
                "cracks",
                "crystal_growth",
                "dla",
                "physarum",
                "vines",
            ],
        },
        BehaviorCategory {
            id: "motion",
            label: "Motion",
            behavior_ids: &[
                "bounce",
                "bubbles",
                "gravity",
                "boids",
                "lava_lamp",
                "orbit",
                "sand_ripples",
            ],
        },
    ]
}

pub fn behavior_catalog() -> &'static [BehaviorCatalogEntry] {
    &[
        BehaviorCatalogEntry {
            id: "none",
            label: "none",
            category_id: "play",
        },
        BehaviorCatalogEntry {
            id: "keys",
            label: "keys",
            category_id: "play",
        },
        BehaviorCatalogEntry {
            id: "sequencer",
            label: "sequencer",
            category_id: "play",
        },
        BehaviorCatalogEntry {
            id: "looper",
            label: "looper",
            category_id: "play",
        },
        BehaviorCatalogEntry {
            id: "weave",
            label: "weave",
            category_id: "play",
        },
        BehaviorCatalogEntry {
            id: "polyrhythm",
            label: "polyrhythm",
            category_id: "rhythm",
        },
        BehaviorCatalogEntry {
            id: "breaks",
            label: "breaks",
            category_id: "rhythm",
        },
        BehaviorCatalogEntry {
            id: "fills",
            label: "fills",
            category_id: "rhythm",
        },
        BehaviorCatalogEntry {
            id: "clave",
            label: "clave",
            category_id: "rhythm",
        },
        BehaviorCatalogEntry {
            id: "groove",
            label: "groove",
            category_id: "rhythm",
        },
        BehaviorCatalogEntry {
            id: "euclid",
            label: "euclid",
            category_id: "rhythm",
        },
        BehaviorCatalogEntry {
            id: "ostinato",
            label: "ostinato",
            category_id: "musical",
        },
        BehaviorCatalogEntry {
            id: "motif",
            label: "motif",
            category_id: "musical",
        },
        BehaviorCatalogEntry {
            id: "canon",
            label: "canon",
            category_id: "musical",
        },
        BehaviorCatalogEntry {
            id: "chords",
            label: "chords",
            category_id: "musical",
        },
        BehaviorCatalogEntry {
            id: "contour",
            label: "contour",
            category_id: "musical",
        },
        BehaviorCatalogEntry {
            id: "cadence",
            label: "cadence",
            category_id: "musical",
        },
        BehaviorCatalogEntry {
            id: "phrase",
            label: "phrase",
            category_id: "musical",
        },
        BehaviorCatalogEntry {
            id: "life",
            label: "life",
            category_id: "cellular",
        },
        BehaviorCatalogEntry {
            id: "brain",
            label: "brain",
            category_id: "cellular",
        },
        BehaviorCatalogEntry {
            id: "cyclic",
            label: "cyclic",
            category_id: "cellular",
        },
        BehaviorCatalogEntry {
            id: "forest_fire",
            label: "forest fire",
            category_id: "cellular",
        },
        BehaviorCatalogEntry {
            id: "predator_prey",
            label: "predator prey",
            category_id: "cellular",
        },
        BehaviorCatalogEntry {
            id: "ant",
            label: "ant",
            category_id: "cellular",
        },
        BehaviorCatalogEntry {
            id: "boids",
            label: "boids",
            category_id: "motion",
        },
        BehaviorCatalogEntry {
            id: "physarum",
            label: "physarum",
            category_id: "growth",
        },
        BehaviorCatalogEntry {
            id: "bounce",
            label: "bounce",
            category_id: "motion",
        },
        BehaviorCatalogEntry {
            id: "bubbles",
            label: "bubbles",
            category_id: "motion",
        },
        BehaviorCatalogEntry {
            id: "coral",
            label: "coral",
            category_id: "growth",
        },
        BehaviorCatalogEntry {
            id: "cracks",
            label: "cracks",
            category_id: "growth",
        },
        BehaviorCatalogEntry {
            id: "crystal_growth",
            label: "crystal growth",
            category_id: "growth",
        },
        BehaviorCatalogEntry {
            id: "dla",
            label: "dla",
            category_id: "growth",
        },
        BehaviorCatalogEntry {
            id: "ink",
            label: "ink",
            category_id: "fields",
        },
        BehaviorCatalogEntry {
            id: "ising",
            label: "ising",
            category_id: "fields",
        },
        BehaviorCatalogEntry {
            id: "kuramoto",
            label: "kuramoto",
            category_id: "fields",
        },
        BehaviorCatalogEntry {
            id: "lightning",
            label: "lightning",
            category_id: "fields",
        },
        BehaviorCatalogEntry {
            id: "wave",
            label: "wave",
            category_id: "fields",
        },
        BehaviorCatalogEntry {
            id: "raindrops",
            label: "raindrops",
            category_id: "fields",
        },
        BehaviorCatalogEntry {
            id: "reaction_diffusion",
            label: "reaction diffusion",
            category_id: "fields",
        },
        BehaviorCatalogEntry {
            id: "rivers",
            label: "rivers",
            category_id: "fields",
        },
        BehaviorCatalogEntry {
            id: "gravity",
            label: "gravity",
            category_id: "motion",
        },
        BehaviorCatalogEntry {
            id: "orbit",
            label: "orbit",
            category_id: "motion",
        },
        BehaviorCatalogEntry {
            id: "lava_lamp",
            label: "lava lamp",
            category_id: "motion",
        },
        BehaviorCatalogEntry {
            id: "sand_ripples",
            label: "sand ripples",
            category_id: "motion",
        },
        BehaviorCatalogEntry {
            id: "fractal_explorer",
            label: "fractal explorer",
            category_id: "geometry",
        },
        BehaviorCatalogEntry {
            id: "maze_growth",
            label: "maze growth",
            category_id: "geometry",
        },
        BehaviorCatalogEntry {
            id: "shapes",
            label: "shapes",
            category_id: "geometry",
        },
        BehaviorCatalogEntry {
            id: "vines",
            label: "vines",
            category_id: "growth",
        },
    ]
}
