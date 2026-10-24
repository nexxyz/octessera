#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteSetCategory {
    Scales,
    ChordTones,
    Pentatonic,
    Symmetric,
}

impl NoteSetCategory {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Scales => "scales",
            Self::ChordTones => "chord_tones",
            Self::Pentatonic => "pentatonic",
            Self::Symmetric => "symmetric",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Scales => "Scales",
            Self::ChordTones => "Chord Tones",
            Self::Pentatonic => "Pentatonic",
            Self::Symmetric => "Symmetric",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteSetDefinition {
    pub category: NoteSetCategory,
    pub id: &'static str,
    pub label: &'static str,
    pub compact_label: &'static str,
    pub intervals: &'static [u8],
}

pub const NOTE_SET_ROOTS: &[&str] = &[
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub const NOTE_SET_CATEGORIES: &[NoteSetCategory] = &[
    NoteSetCategory::Scales,
    NoteSetCategory::ChordTones,
    NoteSetCategory::Pentatonic,
    NoteSetCategory::Symmetric,
];

pub const NOTE_SET_REGISTRY: &[NoteSetDefinition] = &[
    NoteSetDefinition {
        category: NoteSetCategory::Scales,
        id: "chromatic",
        label: "Chromatic",
        compact_label: "Chromatic",
        intervals: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Scales,
        id: "major",
        label: "Major",
        compact_label: "Major",
        intervals: &[0, 2, 4, 5, 7, 9, 11],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Scales,
        id: "natural_minor",
        label: "Natural Minor",
        compact_label: "Nat Minor",
        intervals: &[0, 2, 3, 5, 7, 8, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Scales,
        id: "harmonic_minor",
        label: "Harmonic Minor",
        compact_label: "Harm Minor",
        intervals: &[0, 2, 3, 5, 7, 8, 11],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Scales,
        id: "dorian",
        label: "Dorian",
        compact_label: "Dorian",
        intervals: &[0, 2, 3, 5, 7, 9, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Scales,
        id: "mixolydian",
        label: "Mixolydian",
        compact_label: "Mixolydian",
        intervals: &[0, 2, 4, 5, 7, 9, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::ChordTones,
        id: "major_triad",
        label: "Major Triad",
        compact_label: "Maj Triad",
        intervals: &[0, 4, 7],
    },
    NoteSetDefinition {
        category: NoteSetCategory::ChordTones,
        id: "minor_triad",
        label: "Minor Triad",
        compact_label: "Min Triad",
        intervals: &[0, 3, 7],
    },
    NoteSetDefinition {
        category: NoteSetCategory::ChordTones,
        id: "diminished_triad",
        label: "Diminished Triad",
        compact_label: "Dim Triad",
        intervals: &[0, 3, 6],
    },
    NoteSetDefinition {
        category: NoteSetCategory::ChordTones,
        id: "suspended_second",
        label: "Sus 2",
        compact_label: "Sus 2",
        intervals: &[0, 2, 7],
    },
    NoteSetDefinition {
        category: NoteSetCategory::ChordTones,
        id: "suspended_fourth",
        label: "Sus 4",
        compact_label: "Sus 4",
        intervals: &[0, 5, 7],
    },
    NoteSetDefinition {
        category: NoteSetCategory::ChordTones,
        id: "major_seventh",
        label: "Major 7",
        compact_label: "Maj 7",
        intervals: &[0, 4, 7, 11],
    },
    NoteSetDefinition {
        category: NoteSetCategory::ChordTones,
        id: "dominant_seventh",
        label: "Dominant 7",
        compact_label: "Dom 7",
        intervals: &[0, 4, 7, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::ChordTones,
        id: "minor_seventh",
        label: "Minor 7",
        compact_label: "Min 7",
        intervals: &[0, 3, 7, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::ChordTones,
        id: "major_ninth",
        label: "Major 9",
        compact_label: "Maj 9",
        intervals: &[0, 2, 4, 7, 11],
    },
    NoteSetDefinition {
        category: NoteSetCategory::ChordTones,
        id: "dominant_ninth",
        label: "Dominant 9",
        compact_label: "Dom 9",
        intervals: &[0, 2, 4, 7, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Pentatonic,
        id: "major_pentatonic",
        label: "Major Pentatonic",
        compact_label: "Maj Pent",
        intervals: &[0, 2, 4, 7, 9],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Pentatonic,
        id: "minor_pentatonic",
        label: "Minor Pentatonic",
        compact_label: "Min Pent",
        intervals: &[0, 3, 5, 7, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Pentatonic,
        id: "suspended_pentatonic",
        label: "Suspended Penta",
        compact_label: "Susp Pent",
        intervals: &[0, 2, 5, 7, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Pentatonic,
        id: "hirajoshi_like",
        label: "Hirajoshi-like",
        compact_label: "Hira-like",
        intervals: &[0, 2, 3, 7, 8],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Pentatonic,
        id: "in_sen_like",
        label: "In Sen-like",
        compact_label: "InSen-like",
        intervals: &[0, 1, 5, 7, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Pentatonic,
        id: "iwato_like",
        label: "Iwato-like",
        compact_label: "Iwato-like",
        intervals: &[0, 1, 5, 6, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Symmetric,
        id: "whole_tone",
        label: "Whole Tone",
        compact_label: "Whole Tone",
        intervals: &[0, 2, 4, 6, 8, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Symmetric,
        id: "octatonic_half_whole",
        label: "Octatonic H-W",
        compact_label: "Oct H-W",
        intervals: &[0, 1, 3, 4, 6, 7, 9, 10],
    },
    NoteSetDefinition {
        category: NoteSetCategory::Symmetric,
        id: "octatonic_whole_half",
        label: "Octatonic W-H",
        compact_label: "Oct W-H",
        intervals: &[0, 2, 3, 5, 6, 8, 9, 11],
    },
];

impl NoteSetDefinition {
    pub fn pitch_classes(self, root: &str) -> Option<Vec<i32>> {
        let root = NOTE_SET_ROOTS
            .iter()
            .position(|candidate| *candidate == root)? as i32;
        Some(
            self.intervals
                .iter()
                .map(|interval| (root + i32::from(*interval)).rem_euclid(12))
                .collect(),
        )
    }
}

pub fn note_set_registry() -> &'static [NoteSetDefinition] {
    NOTE_SET_REGISTRY
}

pub fn note_set_ids() -> impl Iterator<Item = &'static str> {
    NOTE_SET_REGISTRY.iter().map(|note_set| note_set.id)
}

pub fn note_set_categories() -> &'static [NoteSetCategory] {
    NOTE_SET_CATEGORIES
}

pub fn note_set_by_id(id: &str) -> Option<&'static NoteSetDefinition> {
    NOTE_SET_REGISTRY.iter().find(|note_set| note_set.id == id)
}

pub fn expand_note_set(id: &str, root: &str) -> Option<Vec<i32>> {
    note_set_by_id(id).and_then(|note_set| note_set.pitch_classes(root))
}

#[cfg(test)]
mod tests;
