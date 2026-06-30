use super::*;
use crate::behavior::{BehaviorContext, DeviceInput, GridInteraction};

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

#[test]
fn every_native_behavior_routes_input_and_tick() {
    for id in list_native_behavior_ids() {
        let behavior = get_native_behavior(id).unwrap();
        let mut context = BehaviorContext::new(120.0);
        let state = behavior.init(Value::Null).unwrap();
        let state = behavior
            .on_input(state, DeviceInput::Other, &mut context)
            .unwrap();
        let state = behavior.on_tick(state, &mut context).unwrap();
        let model = behavior.render_model(&state).unwrap();
        assert_eq!(
            model.cells.len(),
            crate::grid::GRID_WIDTH * crate::grid::GRID_HEIGHT
        );
    }
}

#[test]
fn behavior_metadata_matches_expected_interaction_modes() {
    assert!(!NativeBehavior::None.interpret_input_transitions());
    assert!(!NativeBehavior::Sequencer.interpret_input_transitions());
    assert!(NativeBehavior::Life.interpret_input_transitions());
    assert_eq!(
        NativeBehavior::Keys.grid_interaction(),
        Some(GridInteraction::Momentary)
    );
    assert_eq!(
        NativeBehavior::Looper.grid_interaction(),
        Some(GridInteraction::Momentary)
    );
    assert_eq!(NativeBehavior::Life.grid_interaction(), None);
}

#[test]
fn behavior_state_mismatches_return_errors() {
    let mut context = BehaviorContext::new(120.0);
    let state = NativeBehavior::Life.init(Value::Null).unwrap();
    assert!(NativeBehavior::Glider
        .on_input(state.clone(), DeviceInput::Other, &mut context)
        .is_err());
    assert!(NativeBehavior::Glider
        .on_tick(state.clone(), &mut context)
        .is_err());
    assert!(NativeBehavior::Glider.render_model(&state).is_err());
    assert!(NativeBehavior::Glider.serialize(&state).is_err());
    assert!(NativeBehavior::Glider.config_menu(&state).is_err());
}
