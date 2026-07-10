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
            id: "cellular",
            label: "Cellular",
            behavior_ids: &["ant", "brain", "glider", "life"],
        },
        BehaviorCategory {
            id: "fields",
            label: "Fields",
            behavior_ids: &["raindrops"],
        },
        BehaviorCategory {
            id: "geometry",
            label: "Geometry",
            behavior_ids: &["shapes"],
        },
        BehaviorCategory {
            id: "growth",
            label: "Growth",
            behavior_ids: &["dla"],
        },
        BehaviorCategory {
            id: "motion",
            label: "Motion",
            behavior_ids: &["bounce"],
        },
        BehaviorCategory {
            id: "play",
            label: "Play",
            behavior_ids: &["keys", "looper", "none", "sequencer"],
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
            id: "life",
            label: "life",
            category_id: "cellular",
        },
        BehaviorCatalogEntry {
            id: "glider",
            label: "glider",
            category_id: "cellular",
        },
        BehaviorCatalogEntry {
            id: "brain",
            label: "brain",
            category_id: "cellular",
        },
        BehaviorCatalogEntry {
            id: "ant",
            label: "ant",
            category_id: "cellular",
        },
        BehaviorCatalogEntry {
            id: "bounce",
            label: "bounce",
            category_id: "motion",
        },
        BehaviorCatalogEntry {
            id: "dla",
            label: "dla",
            category_id: "growth",
        },
        BehaviorCatalogEntry {
            id: "raindrops",
            label: "raindrops",
            category_id: "fields",
        },
        BehaviorCatalogEntry {
            id: "shapes",
            label: "shapes",
            category_id: "geometry",
        },
    ]
}
