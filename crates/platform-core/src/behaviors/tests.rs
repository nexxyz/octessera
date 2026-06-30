use super::*;

#[test]
fn lists_and_resolves_native_behaviors() {
    assert_eq!(
        list_native_behavior_ids(),
        &[
            "none",
            "life",
            "sequencer",
            "keys",
            "looper",
            "brain",
            "ant",
            "bounce",
            "shapes",
            "raindrops",
            "dla",
            "glider",
        ]
    );
    assert_eq!(get_native_behavior("life"), Some(NativeBehavior::Life));
    assert_eq!(
        get_native_behavior("sequencer"),
        Some(NativeBehavior::Sequencer)
    );
    assert_eq!(get_native_behavior("keys"), Some(NativeBehavior::Keys));
    assert_eq!(get_native_behavior("looper"), Some(NativeBehavior::Looper));
    assert_eq!(get_native_behavior("dla"), Some(NativeBehavior::Dla));
    assert_eq!(get_native_behavior("missing"), None);
}

#[test]
fn every_native_behavior_supports_runtime_contract() {
    for id in list_native_behavior_ids() {
        let behavior = get_native_behavior(id).unwrap();
        let state = behavior.init(Value::Null).unwrap();
        let model = behavior.render_model(&state).unwrap();
        assert_eq!(
            model.cells.len(),
            crate::grid::GRID_WIDTH * crate::grid::GRID_HEIGHT
        );
        let serialized = behavior.serialize(&state).unwrap();
        assert!(serialized.get("generation").is_none());
        assert!(serialized.get("tickCounter").is_none());
        let restored = behavior.deserialize(serialized).unwrap();
        let _ = behavior.config_menu(&restored).unwrap();
    }
}
