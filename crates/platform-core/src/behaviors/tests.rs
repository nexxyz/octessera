use super::*;
use crate::behavior::{BehaviorContext, CellTriggerType, DeviceInput, GridInteraction};
use serde_json::Value;
use std::collections::HashSet;

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
            "cyclic",
            "forest_fire",
            "predator_prey",
            "ant",
            "bounce",
            "bubbles",
            "gravity",
            "boids",
            "lava_lamp",
            "orbit",
            "sand_ripples",
            "fractal_explorer",
            "maze_growth",
            "shapes",
            "ink",
            "ising",
            "kuramoto",
            "lightning",
            "raindrops",
            "reaction_diffusion",
            "rivers",
            "wave",
            "cracks",
            "coral",
            "crystal_growth",
            "dla",
            "physarum",
            "vines",
            "weave",
            "polyrhythm",
            "breaks",
            "fills",
            "clave",
            "groove",
            "euclid",
            "ostinato",
            "motif",
            "canon",
            "chords",
            "contour",
            "cadence",
            "phrase",
        ]
    );
    assert_eq!(get_native_behavior("arp"), None);
    assert_eq!(get_native_behavior("phrase"), Some(NativeBehavior::Phrase));
    assert_eq!(get_native_behavior("life"), Some(NativeBehavior::Life));
    assert_eq!(get_native_behavior("glider"), None);
    assert_eq!(
        get_native_behavior("sequencer"),
        Some(NativeBehavior::Sequencer)
    );
    assert_eq!(get_native_behavior("keys"), Some(NativeBehavior::Keys));
    assert_eq!(get_native_behavior("looper"), Some(NativeBehavior::Looper));
    assert_eq!(
        get_native_behavior("bubbles"),
        Some(NativeBehavior::Bubbles)
    );
    assert_eq!(get_native_behavior("dla"), Some(NativeBehavior::Dla));
    assert_eq!(
        get_native_behavior("crystal_growth"),
        Some(NativeBehavior::CrystalGrowth)
    );
    assert_eq!(get_native_behavior("crystal"), None);
    assert_eq!(get_native_behavior("crystals"), None);
    assert_eq!(
        get_native_behavior("lightning"),
        Some(NativeBehavior::Lightning)
    );
    assert_eq!(get_native_behavior("bolt"), None);
    assert_eq!(
        get_native_behavior("kuramoto"),
        Some(NativeBehavior::Kuramoto)
    );
    assert_eq!(get_native_behavior("kuramoto_oscillator"), None);
    assert_eq!(get_native_behavior("cyclic"), Some(NativeBehavior::Cyclic));
    assert_eq!(get_native_behavior("cyclic_ca"), None);
    assert_eq!(get_native_behavior("wave"), Some(NativeBehavior::Wave));
    assert_eq!(get_native_behavior("waves"), None);
    assert_eq!(
        get_native_behavior("gravity"),
        Some(NativeBehavior::Gravity)
    );
    assert_eq!(
        get_native_behavior("lava_lamp"),
        Some(NativeBehavior::LavaLamp)
    );
    assert_eq!(get_native_behavior("sand"), None);
    assert_eq!(get_native_behavior("boids"), Some(NativeBehavior::Boids));
    assert_eq!(get_native_behavior("boid"), None);
    assert_eq!(get_native_behavior("orbit"), Some(NativeBehavior::Orbit));
    assert_eq!(
        get_native_behavior("sand_ripples"),
        Some(NativeBehavior::SandRipples)
    );
    assert_eq!(get_native_behavior("orbits"), None);
    assert_eq!(
        get_native_behavior("fractal_explorer"),
        Some(NativeBehavior::FractalExplorer)
    );
    assert_eq!(
        get_native_behavior("maze_growth"),
        Some(NativeBehavior::MazeGrowth)
    );
    assert_eq!(get_native_behavior("fractal"), None);
    assert_eq!(get_native_behavior("ink"), Some(NativeBehavior::Ink));
    assert_eq!(get_native_behavior("inks"), None);
    assert_eq!(get_native_behavior("ising"), Some(NativeBehavior::Ising));
    assert_eq!(get_native_behavior("magnet"), None);
    assert_eq!(
        get_native_behavior("reaction_diffusion"),
        Some(NativeBehavior::ReactionDiffusion)
    );
    assert_eq!(get_native_behavior("gray_scott"), None);
    assert_eq!(get_native_behavior("rivers"), Some(NativeBehavior::Rivers));
    assert_eq!(get_native_behavior("river"), None);
    assert_eq!(get_native_behavior("cracks"), Some(NativeBehavior::Cracks));
    assert_eq!(get_native_behavior("crack"), None);
    assert_eq!(get_native_behavior("coral"), Some(NativeBehavior::Coral));
    assert_eq!(get_native_behavior("reef"), None);
    assert_eq!(
        get_native_behavior("physarum"),
        Some(NativeBehavior::Physarum)
    );
    assert_eq!(get_native_behavior("slime"), None);
    assert_eq!(get_native_behavior("vines"), Some(NativeBehavior::Vines));
    assert_eq!(get_native_behavior("vine"), None);
    assert_eq!(
        get_native_behavior("predator_prey"),
        Some(NativeBehavior::PredatorPrey)
    );
    assert_eq!(get_native_behavior("predator"), None);
    assert_eq!(
        get_native_behavior("forest_fire"),
        Some(NativeBehavior::ForestFire)
    );
    assert_eq!(get_native_behavior("forest"), None);
    assert_eq!(get_native_behavior("missing"), None);
}

#[test]
fn new_pattern_behaviors_toggle_grid_and_round_trip() {
    let mut default_frames = HashSet::new();
    for id in [
        "weave",
        "polyrhythm",
        "breaks",
        "fills",
        "clave",
        "groove",
        "euclid",
        "ostinato",
        "motif",
        "canon",
        "chords",
        "contour",
        "cadence",
        "phrase",
    ] {
        let behavior = get_native_behavior(id).unwrap();
        let mut context = BehaviorContext::new(120.0);
        let state = behavior.init(Value::Null).unwrap();
        let before = behavior.render_model(&state).unwrap().cells;
        default_frames.insert(before.clone());
        let toggled = behavior
            .on_input(state, DeviceInput::GridPress { x: 1, y: 2 }, &mut context)
            .unwrap();
        let after = behavior.render_model(&toggled).unwrap().cells;
        assert_ne!(before, after, "{id} grid input should alter cells");
        let restored = behavior
            .deserialize(behavior.serialize(&toggled).unwrap())
            .unwrap();
        let restored_model = behavior.render_model(&restored).unwrap();
        assert_eq!(restored_model.cells, after);
        assert!(restored_model
            .trigger_types
            .unwrap()
            .iter()
            .all(|trigger| matches!(trigger, CellTriggerType::Stable | CellTriggerType::None)));

        let configured = behavior
            .init(serde_json::json!({
                "densityPct": 999,
                "variationPct": 999,
                "cycleLength": 999,
                "seed": 12345
            }))
            .unwrap();
        let saved = behavior.serialize(&configured).unwrap();
        assert_eq!(saved["densityPct"], 80);
        assert_eq!(saved["variationPct"], 100);
        assert_eq!(saved["cycleLength"], 32);
        assert_eq!(saved["seed"], 9999);
        let clamped_low = behavior
            .init(serde_json::json!({
                "densityPct": -1,
                "variationPct": -1,
                "cycleLength": 0,
                "seed": 0
            }))
            .unwrap();
        let low_saved = behavior.serialize(&clamped_low).unwrap();
        assert_eq!(low_saved["densityPct"], 10);
        assert_eq!(low_saved["variationPct"], 0);
        assert_eq!(low_saved["cycleLength"], 4);
        assert!(low_saved["seed"].as_u64().unwrap_or(0) > 0);
        let configured_model = behavior.render_model(&configured).unwrap();
        for (visible, trigger) in configured_model
            .cells
            .iter()
            .zip(configured_model.trigger_types.unwrap())
        {
            assert!(matches!(
                (*visible, trigger),
                (true, CellTriggerType::Activate) | (false, CellTriggerType::None)
            ));
        }
    }
    assert!(default_frames.len() >= 12);
}

#[test]
fn behavior_catalog_matches_registry() {
    let list_ids = list_native_behavior_ids()
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut catalog_ids = HashSet::new();
    let category_ids = behavior_categories()
        .iter()
        .map(|category| category.id)
        .collect::<HashSet<_>>();

    for entry in behavior_catalog() {
        assert!(
            catalog_ids.insert(entry.id),
            "duplicate behavior catalog id"
        );
        assert!(
            category_ids.contains(entry.category_id),
            "catalog entry has unknown category"
        );
        assert_eq!(
            get_native_behavior(entry.id).map(|behavior| behavior.id()),
            Some(entry.id)
        );
    }

    assert_eq!(catalog_ids, list_ids);

    for category in behavior_categories() {
        assert!(
            category
                .behavior_ids
                .iter()
                .all(|behavior_id| catalog_ids.contains(behavior_id)),
            "category includes unknown behavior id"
        );
    }
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
    assert!(NativeBehavior::Sequencer
        .on_input(state.clone(), DeviceInput::Other, &mut context)
        .is_err());
    assert!(NativeBehavior::Sequencer
        .on_tick(state.clone(), &mut context)
        .is_err());
    assert!(NativeBehavior::Sequencer.render_model(&state).is_err());
    assert!(NativeBehavior::Sequencer.serialize(&state).is_err());
    assert!(NativeBehavior::Sequencer.config_menu(&state).is_err());
}
