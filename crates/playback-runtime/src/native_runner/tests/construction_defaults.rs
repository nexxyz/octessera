use super::super::construction_seed::NativeRunnerConstructionSeed;
use super::*;
use crate::native_menu::NativeMenuValue;

#[test]
fn sequencer_construction_projects_seeded_defaults_into_runtime_and_menu() {
    let defaults = NativeRunnerConfig::default();
    let config = NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        bpm: 137.0,
        swing_pct: 90,
        sample_builtin_favourite_dirs: vec!["library/drums".into()],
        global_sound: GlobalSoundConfig {
            velocity_scale_pct: 82,
            velocity_curve: VelocityCurve::Hard,
            note_length_ms: 240,
        },
        ..defaults
    };
    let behavior = platform_core::get_native_behavior(&config.behavior_id).unwrap();
    let runner = NativeRunner::new(config.clone()).unwrap();
    let expected_seed = NativeRunnerConstructionSeed::new(
        &config,
        behavior,
        NativeUiState::default(),
        Instant::now(),
    );
    let expected_menu = expected_seed.initial_menu_config(&runner.preset_draft_name);

    assert_eq!(runner.behavior.id(), expected_menu.behavior_id);
    assert_eq!(runner.transport.bpm, config.bpm);
    assert_eq!(runner.transport.swing_pct, 75);
    assert_eq!(runner.layer_behavior_ids, expected_seed.layer_behavior_ids);
    assert_eq!(runner.layer_names, expected_seed.layer_names);
    assert_eq!(runner.instruments, expected_seed.instruments);
    assert_eq!(
        runner.sample_builtin_favourite_dirs,
        expected_seed.sample_builtin_favourite_dirs
    );
    assert_eq!(runner.display.ui.master_volume, expected_menu.master_volume);
    assert_eq!(
        runner.display.ui.display_brightness,
        expected_menu.display_brightness
    );

    let world_labels = runner.menu.root.children[0]
        .children
        .iter()
        .map(|item| item.label.clone())
        .collect::<Vec<_>>();
    assert_eq!(world_labels, expected_menu.layer_labels);
    assert_eq!(
        runner.menu.item_for_key("behaviorId").unwrap().label,
        format!("Behavior: {}", expected_menu.behavior_id)
    );
    assert_eq!(
        runner.menu.selected_master_volume(),
        Some(expected_menu.master_volume)
    );
    assert_eq!(
        runner.menu.selected_display_brightness(),
        Some(expected_menu.display_brightness)
    );
    let Some(NativeMenuValue::Text { value, .. }) = runner
        .menu
        .item_for_key("system.draftName")
        .map(|item| item.value)
    else {
        panic!("initial draft name menu value");
    };
    assert_eq!(value, runner.preset_draft_name);
}

#[test]
fn non_default_transport_and_display_defaults_are_seeded_without_menu_rebuild_comparison() {
    let defaults = NativeRunnerConfig::default();
    let config = NativeRunnerConfig {
        behavior_id: "keys".into(),
        bpm: 61.0,
        swing_pct: 12,
        audio_output_buffer_frames: 1024,
        ..defaults
    };
    let behavior = platform_core::get_native_behavior(&config.behavior_id).unwrap();
    let runner = NativeRunner::new(config.clone()).unwrap();
    let expected_seed = NativeRunnerConstructionSeed::new(
        &config,
        behavior,
        NativeUiState::default(),
        Instant::now(),
    );
    let expected_menu = expected_seed.initial_menu_config(&runner.preset_draft_name);

    assert_eq!(runner.behavior.id(), "keys");
    assert_eq!(runner.transport.bpm, 61.0);
    assert_eq!(runner.transport.swing_pct, 12);
    assert_eq!(runner.audio_output_buffer_frames, 1024);
    assert_eq!(runner.active_layer_index, expected_menu.active_layer_index);
    assert_eq!(
        runner.menu.item_for_key("behaviorId").unwrap().label,
        "Behavior: keys"
    );
    assert_eq!(
        runner.menu.selected_sync_source(),
        Some(expected_menu.sync_source)
    );
    assert_eq!(
        runner.menu.selected_button_brightness(),
        Some(expected_menu.button_brightness)
    );
}
