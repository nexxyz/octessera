use super::super::construction_seed::NativeRunnerConstructionSeed;
use super::*;
use crate::native_menu::NativeMenuValue;

#[test]
fn sequencer_construction_projects_seeded_defaults_into_runtime_and_menu() {
    let defaults = NativeRunnerConfig::default();
    let config = NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        bpm: 137.0,
        swing_a: 9,
        swing_b: 0,
        sample_builtin_favourite_dirs: vec!["library/drums".into()],
        global_sound: GlobalSoundConfig {
            velocity_scale_pct: 82,
            velocity_curve: VelocityCurve::Hard,
            note_length_ms: 240,
        },
        ..defaults
    };
    let runner = NativeRunner::new(config.clone()).unwrap();
    let expected_seed =
        NativeRunnerConstructionSeed::new(&config, NativeUiState::default(), Instant::now());

    assert_eq!(runner.active_behavior().id(), "sequencer");
    assert_eq!(runner.transport.bpm, config.bpm);
    assert_eq!((runner.transport.swing_a, runner.transport.swing_b), (7, 1));
    assert!(runner
        .layers
        .iter()
        .all(|layer| layer.behavior_id == "sequencer" && layer.name == "sequencer"));
    assert_eq!(runner.instruments, expected_seed.instruments);
    assert_eq!(
        runner.sample_builtin_favourite_dirs,
        expected_seed.sample_builtin_favourite_dirs
    );
    assert_eq!(
        runner.display.ui.master_volume,
        expected_seed.ui.master_volume
    );
    assert_eq!(
        runner.display.ui.display_brightness,
        expected_seed.ui.display_brightness
    );

    let world_labels = runner.menu.root.children[0]
        .children
        .iter()
        .map(|item| item.label.clone())
        .collect::<Vec<_>>();
    let mut expected_world_labels = (1..=LAYER_COUNT)
        .map(|layer| format!("L{layer}: sequencer"))
        .collect::<Vec<_>>();
    expected_world_labels.push("Seed".into());
    assert_eq!(world_labels, expected_world_labels);
    assert_eq!(
        runner.menu.item_for_key("behaviorId").unwrap().label,
        "Beh.: sequencer"
    );
    assert_eq!(
        runner.menu.selected_master_volume(),
        Some(expected_seed.ui.master_volume)
    );
    assert_eq!(
        runner.menu.selected_display_brightness(),
        Some(expected_seed.ui.display_brightness)
    );
    let Some(NativeMenuValue::Text { value, .. }) = runner
        .menu
        .item_for_key("system.draftName")
        .map(|item| item.value)
    else {
        panic!("initial draft name menu value");
    };
    assert_eq!(value, runner.patch_store.preset_draft_name);
}

#[test]
fn non_default_transport_and_display_defaults_are_seeded_without_menu_rebuild_comparison() {
    let defaults = NativeRunnerConfig::default();
    let config = NativeRunnerConfig {
        behavior_id: "keys".into(),
        bpm: 61.0,
        swing_a: 5,
        swing_b: 3,
        ..defaults
    };
    let runner = NativeRunner::new(config.clone()).unwrap();
    let expected_seed =
        NativeRunnerConstructionSeed::new(&config, NativeUiState::default(), Instant::now());

    assert_eq!(runner.active_behavior().id(), "keys");
    assert_eq!(runner.transport.bpm, 61.0);
    assert_eq!((runner.transport.swing_a, runner.transport.swing_b), (5, 3));
    assert_eq!(runner.active_layer_index, expected_seed.active_layer_index);
    assert_eq!(
        runner.menu.item_for_key("behaviorId").unwrap().label,
        "Beh.: keys"
    );
    assert_eq!(
        runner.menu.selected_sync_source(),
        Some(expected_seed.sync_source)
    );
    assert_eq!(
        runner.menu.selected_button_brightness(),
        Some(expected_seed.ui.button_brightness)
    );
}
