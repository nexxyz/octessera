//! Picking a behavior sets the layer's Scan Mode and Note Preview from the behavior's play style;
//! both stay as edited until the next pick, and a patch load keeps what the patch saved.

use super::*;
use platform_core::{behavior_catalog, PlayStyle};

fn expected(style: PlayStyle) -> (ScanMode, bool) {
    match style {
        PlayStyle::GridSequencer => (ScanMode::Scanning, true),
        PlayStyle::Generative => (ScanMode::None, true),
        PlayStyle::ImmediatePlay | PlayStyle::Game => (ScanMode::None, false),
    }
}

fn link_of(runner: &NativeRunner, layer: usize) -> (ScanMode, bool) {
    let link = &runner.layers[layer].link;
    (link.scan_mode, link.note_preview)
}

#[test]
fn every_behavior_pick_sets_its_play_styles_scan_mode_and_note_preview() {
    for entry in behavior_catalog() {
        let behavior = platform_core::get_native_behavior(entry.id).unwrap();
        let start = if entry.id == "life" { "keys" } else { "life" };
        for layer in [0, 2] {
            let mut runner = NativeRunner::new(NativeRunnerConfig {
                behavior_id: start.into(),
                ..NativeRunnerConfig::default()
            })
            .unwrap();
            runner.select_layer_behavior(layer, start).unwrap();
            runner.layers[layer].link.scan_axis = ScanAxis::Rows;
            runner.layers[layer].link.scan_direction = ScanDirection::Reverse;
            runner.select_layer_behavior(layer, entry.id).unwrap();
            assert_eq!(
                runner.layers[layer].link.scan_direction,
                ScanDirection::Forward
            );
            assert_eq!(
                link_of(&runner, layer),
                expected(behavior.play_style()),
                "{} on layer {layer}",
                entry.id
            );
            let scans = runner.layers[layer].link.scan_mode == ScanMode::Scanning;
            if scans {
                assert_eq!(runner.layers[layer].link.scan_axis, ScanAxis::Columns);
            }
            let menu_value = runner
                .menu
                .value_for_key(&format!("layers.{layer}.link.notePreview"));
            if let Some(value) = menu_value {
                assert_eq!(value, runner.layers[layer].link.note_preview.to_string());
            }
        }
    }
}

#[test]
fn an_edit_after_a_pick_sticks_until_the_next_pick() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.select_behavior("sequencer").unwrap();
    assert_eq!(link_of(&runner, 0), (ScanMode::Scanning, true));
    runner.menu.rebuild(runner.menu_config());
    runner
        .edit_menu_key("layers.0.link.notePreview", -1)
        .unwrap();
    runner.edit_menu_key("layers.0.link.scanMode", -1).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    for _ in 0..4 {
        runner
            .send(HostMessage::TransportPulseStep {
                pulses: 24,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
    }
    assert_eq!(link_of(&runner, 0), (ScanMode::None, false));
    runner.select_behavior("euclid").unwrap();
    assert_eq!(link_of(&runner, 0), (ScanMode::Scanning, true));
}

#[test]
fn a_patch_load_keeps_the_saved_scan_mode_and_note_preview() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let mut payload = runner.config_payload();
    let layer = &mut payload["runtimeConfig"]["layers"][1];
    layer["build"]["behaviorId"] = json!("sequencer");
    layer["build"]["behaviorConfig"] = Value::Null;
    layer["build"]["savedState"] = Value::Null;
    layer["link"]["scanMode"] = json!("none");
    layer["link"]["notePreview"] = json!(false);
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.layers[1].behavior_id, "sequencer");
    assert_eq!(link_of(&runner, 1), (ScanMode::None, false));
}

/// Picking a behavior that moves the layer onto a scan drains what its old setup held.
#[test]
fn a_pick_that_changes_the_scan_mode_drains_the_layer_first() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    hold_key(&mut runner, 0, 60);
    runner.select_behavior("sequencer").unwrap();
    assert_eq!(runner.held_keys.held_count(), 0);
}
