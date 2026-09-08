use super::*;

#[test]
pub(crate) fn note_set_help_targets_use_exact_folder_and_leaf_rows() {
    let menu = NativeMenuModel::new(config());
    let targets = menu.help_targets();

    for category in platform_core::note_set_categories() {
        let key = format!("key:note_set.category.{}", category.id());
        let target = targets
            .iter()
            .find(|target| target.key == key)
            .unwrap_or_else(|| panic!("missing note set category help target {key}"));
        let entry = crate::native_help::resolve_native_help_entry(target)
            .unwrap_or_else(|| panic!("missing note set category help row {key}"));
        assert_eq!(entry.key, key);
        assert_eq!(entry.kind, "group");
    }

    for note_set in platform_core::note_set_registry() {
        let key = format!("action:note_set_select:{}", note_set.id);
        let target = targets
            .iter()
            .find(|target| target.key == key)
            .unwrap_or_else(|| panic!("missing note set leaf help target {key}"));
        let entry = crate::native_help::resolve_native_help_entry(target)
            .unwrap_or_else(|| panic!("missing note set leaf help row {key}"));
        assert_eq!(entry.key, key);
        assert_eq!(entry.kind, "action");
        let copy = format!("{} {}", entry.line1, entry.line2);
        assert!(copy.contains("allowed pitch classes"), "{key}: {copy}");
        if note_set.category == platform_core::NoteSetCategory::ChordTones {
            assert!(
                copy.contains("simultaneous chords or choose inversions"),
                "{key}: {copy}"
            );
        }
        if matches!(note_set.id, "hirajoshi_like" | "in_sen_like" | "iwato_like") {
            assert!(
                copy.contains("common Western 12-TET approximation"),
                "{key}: {copy}"
            );
            assert!(copy.contains("forms vary"), "{key}: {copy}");
        }
    }
}
