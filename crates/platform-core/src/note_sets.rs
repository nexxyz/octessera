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
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_has_the_approved_depth_first_table() {
        let expected = [
            ("chromatic", NoteSetCategory::Scales),
            ("major", NoteSetCategory::Scales),
            ("natural_minor", NoteSetCategory::Scales),
            ("harmonic_minor", NoteSetCategory::Scales),
            ("dorian", NoteSetCategory::Scales),
            ("mixolydian", NoteSetCategory::Scales),
            ("major_triad", NoteSetCategory::ChordTones),
            ("minor_triad", NoteSetCategory::ChordTones),
            ("diminished_triad", NoteSetCategory::ChordTones),
            ("suspended_second", NoteSetCategory::ChordTones),
            ("suspended_fourth", NoteSetCategory::ChordTones),
            ("major_seventh", NoteSetCategory::ChordTones),
            ("dominant_seventh", NoteSetCategory::ChordTones),
            ("minor_seventh", NoteSetCategory::ChordTones),
            ("major_ninth", NoteSetCategory::ChordTones),
            ("dominant_ninth", NoteSetCategory::ChordTones),
            ("major_pentatonic", NoteSetCategory::Pentatonic),
            ("minor_pentatonic", NoteSetCategory::Pentatonic),
            ("suspended_pentatonic", NoteSetCategory::Pentatonic),
            ("hirajoshi_like", NoteSetCategory::Pentatonic),
            ("in_sen_like", NoteSetCategory::Pentatonic),
            ("iwato_like", NoteSetCategory::Pentatonic),
            ("whole_tone", NoteSetCategory::Symmetric),
            ("octatonic_half_whole", NoteSetCategory::Symmetric),
            ("octatonic_whole_half", NoteSetCategory::Symmetric),
        ];
        assert_eq!(NOTE_SET_REGISTRY.len(), expected.len());
        assert_eq!(
            NOTE_SET_REGISTRY
                .iter()
                .map(|set| (set.id, set.category))
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            NOTE_SET_REGISTRY
                .iter()
                .map(|set| set.id)
                .collect::<HashSet<_>>()
                .len(),
            25
        );
        assert_eq!(
            NOTE_SET_ROOTS,
            &["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
        );
        let root_numbers = NOTE_SET_ROOTS
            .iter()
            .enumerate()
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(root_numbers, (0..12).collect::<Vec<_>>());
        assert_eq!(NOTE_SET_ROOTS.iter().collect::<HashSet<_>>().len(), 12);
        for set in NOTE_SET_REGISTRY {
            assert!(!set.id.is_empty());
            assert!(!set.label.is_empty());
            assert!(!set.compact_label.is_empty());
            assert!(set.compact_label.chars().count() <= 10);
            assert!(!set.intervals.is_empty());
            assert!(set.intervals.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(set.intervals.iter().all(|interval| *interval < 12));
        }
    }

    #[test]
    fn categories_are_exact_and_non_empty() {
        assert_eq!(
            NOTE_SET_CATEGORIES,
            &[
                NoteSetCategory::Scales,
                NoteSetCategory::ChordTones,
                NoteSetCategory::Pentatonic,
                NoteSetCategory::Symmetric,
            ]
        );
        for category in NOTE_SET_CATEGORIES {
            assert!(NOTE_SET_REGISTRY
                .iter()
                .any(|note_set| note_set.category == *category));
        }
    }

    #[test]
    fn approved_intervals_and_major_triad_are_exact() {
        let expected = [
            ("chromatic", &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..]),
            ("major", &[0, 2, 4, 5, 7, 9, 11][..]),
            ("natural_minor", &[0, 2, 3, 5, 7, 8, 10][..]),
            ("harmonic_minor", &[0, 2, 3, 5, 7, 8, 11][..]),
            ("dorian", &[0, 2, 3, 5, 7, 9, 10][..]),
            ("mixolydian", &[0, 2, 4, 5, 7, 9, 10][..]),
            ("major_triad", &[0, 4, 7][..]),
            ("minor_triad", &[0, 3, 7][..]),
            ("diminished_triad", &[0, 3, 6][..]),
            ("suspended_second", &[0, 2, 7][..]),
            ("suspended_fourth", &[0, 5, 7][..]),
            ("major_seventh", &[0, 4, 7, 11][..]),
            ("dominant_seventh", &[0, 4, 7, 10][..]),
            ("minor_seventh", &[0, 3, 7, 10][..]),
            ("major_ninth", &[0, 2, 4, 7, 11][..]),
            ("dominant_ninth", &[0, 2, 4, 7, 10][..]),
            ("major_pentatonic", &[0, 2, 4, 7, 9][..]),
            ("minor_pentatonic", &[0, 3, 5, 7, 10][..]),
            ("suspended_pentatonic", &[0, 2, 5, 7, 10][..]),
            ("hirajoshi_like", &[0, 2, 3, 7, 8][..]),
            ("in_sen_like", &[0, 1, 5, 7, 10][..]),
            ("iwato_like", &[0, 1, 5, 6, 10][..]),
            ("whole_tone", &[0, 2, 4, 6, 8, 10][..]),
            ("octatonic_half_whole", &[0, 1, 3, 4, 6, 7, 9, 10][..]),
            ("octatonic_whole_half", &[0, 2, 3, 5, 6, 8, 9, 11][..]),
        ];
        for (id, intervals) in expected {
            assert_eq!(note_set_by_id(id).unwrap().intervals, intervals);
        }
        assert_eq!(expand_note_set("major_triad", "C"), Some(vec![0, 4, 7]));
        assert_eq!(expand_note_set("major_triad", "D"), Some(vec![2, 6, 9]));
        assert_eq!(expand_note_set("missing", "C"), None);
        assert_eq!(expand_note_set("major_triad", "H"), None);
    }

    #[test]
    fn every_root_expands_by_exact_interval_arithmetic() {
        for note_set in NOTE_SET_REGISTRY {
            for (root_index, root) in NOTE_SET_ROOTS.iter().enumerate() {
                let expected = note_set
                    .intervals
                    .iter()
                    .map(|interval| ((root_index as u8 + *interval) % 12) as i32)
                    .collect::<Vec<_>>();
                assert_eq!(note_set.pitch_classes(root), Some(expected));
            }
        }
    }

    #[test]
    fn approximation_compact_labels_are_exact() {
        assert_eq!(
            note_set_by_id("hirajoshi_like").unwrap().compact_label,
            "Hira-like"
        );
        assert_eq!(
            note_set_by_id("in_sen_like").unwrap().compact_label,
            "InSen-like"
        );
        assert_eq!(
            note_set_by_id("iwato_like").unwrap().compact_label,
            "Iwato-like"
        );
    }

    #[test]
    fn every_root_expands_every_set_to_pitch_classes() {
        for set in NOTE_SET_REGISTRY {
            for root in NOTE_SET_ROOTS {
                let expanded = set.pitch_classes(root).unwrap();
                assert_eq!(expanded.len(), set.intervals.len());
                assert!(expanded
                    .iter()
                    .all(|pitch_class| (0..12).contains(pitch_class)));
            }
        }
    }
}
