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
