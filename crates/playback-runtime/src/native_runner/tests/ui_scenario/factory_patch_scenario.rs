use super::device_driver::DeviceDriver;
use super::visible_menu_driver::VisibleMenuDriver;
use super::{factory_patch_configuration, factory_patch_playback};
use crate::{NativeRunner, NativeRunnerConfig, RuntimeStoreResult};
use serde_json::{json, Value};

const FACTORY_PATCH_SEQUENCE: &[&str] = &[
    "System > Saves > Load Empty > Confirm Load Empty",
    "Layer 1 double-line cross grid paint",
    "Layer 2 row-pattern grid paint",
    "Build L1 Life 16th no random spawn",
    "Build L2 Sequencer 8th",
    "Build L3 Looper 8th",
    "Route BPM and layer note/scanning routes",
    "Shape synth/sampler/hold setup and aux assignments",
    "FX Bus 1 Delay + Duck target I2 amount 60",
    "Play X/Y bindings and FX block assignments",
    "Build menu generated values are walked, saved, reloaded, and walked again",
    "Transport, mute, looper, X/Y, FX, aux assertions",
];

pub(super) fn run() {
    let mut device = DeviceDriver::new();
    for step in FACTORY_PATCH_SEQUENCE {
        device.note_step(*step);
    }

    factory_patch_configuration::clear_all_from_visible_ui(&mut device);
    factory_patch_configuration::configure_worlds_and_paint_from_visible_ui(&mut device);
    factory_patch_configuration::configure_pulses_from_visible_ui(&mut device);
    factory_patch_configuration::configure_tones_from_visible_ui(&mut device);
    factory_patch_configuration::configure_aux_xy_and_sparks_fx_from_visible_ui(&mut device);
    assert_build_menu_generated_values(&mut device);
    save_and_reload_test_json_then_recheck_build_menu(&mut device);
    assert_factory_patch_matches_expected_fixture(&device);
    factory_patch_playback::assert_configured_patch_emits(&mut device);
    factory_patch_playback::assert_mute_looper_xy_fx_and_aux_paths(&mut device);
}

fn assert_build_menu_generated_values(device: &mut DeviceDriver) {
    let mut menu = VisibleMenuDriver::new(device);
    menu.back_to_root();
    menu.open_group("Build");
    menu.open_group("L1:");
    menu.expect_visible_value("Behavior", "life");
    menu.expect_visible_value("Step Rate", "1/16");
    menu.expect_visible_value("Spawn Count", "0");
    menu.expect_visible_value("Spawn Interval", "1");
    menu.back();

    menu.open_group("L2:");
    menu.expect_visible_value("Behavior", "sequencer");
    menu.expect_visible_value("Step Rate", "1/8");
    menu.back();

    menu.open_group("L3:");
    menu.expect_visible_value("Behavior", "looper");
    menu.expect_visible_value("Step Rate", "1/8");
    menu.expect_visible_value("Length", "16");
    menu.select_visible("Punch");
    menu.select_visible("Clear Loop");
    menu.back_to_root();
}

fn save_and_reload_test_json_then_recheck_build_menu(device: &mut DeviceDriver) {
    save_visible_preset_as_test_json(device);
    let (name, payload) = device
        .latest_saved_preset()
        .unwrap_or_else(|| device.fail("Save As did not emit a StoreSavePreset effect"));
    if name != "test.json" {
        device.fail(&format!("Save As emitted unexpected preset name `{name}`"));
    }
    device.send_store_result(RuntimeStoreResult::SavePresetResult {
        name: name.clone(),
        outcome: "saved".into(),
    });

    let mut reloaded = DeviceDriver::new();
    reloaded.send_store_result(RuntimeStoreResult::ListPresetsResult {
        names: vec![name.clone()],
    });
    load_visible_preset(&mut reloaded, &name);
    if reloaded.latest_load_preset_request() != Some(name.as_str()) {
        reloaded.fail("Load menu did not request test.json");
    }
    reloaded.send_store_result(RuntimeStoreResult::LoadPresetResult {
        name,
        payload: Some(payload),
    });
    assert_build_menu_generated_values(&mut reloaded);
    assert_factory_patch_matches_expected_fixture(&reloaded);
}

fn save_visible_preset_as_test_json(device: &mut DeviceDriver) {
    device.set_preset_draft_name("test.json");
    let mut menu = VisibleMenuDriver::new(device);
    menu.open_group("System");
    menu.open_group("Saves");
    menu.open_group("Library");
    menu.open_group("Save As");
    menu.expect_visible_value("Name", "test.json");
    menu.activate_action("Save");
    menu.confirm("Confirm Save");
    menu.back_to_root();
}

fn load_visible_preset(device: &mut DeviceDriver, name: &str) {
    let mut menu = VisibleMenuDriver::new(device);
    menu.open_group("System");
    menu.open_group("Saves");
    menu.open_group("Library");
    menu.open_group("Load");
    menu.activate_action(name);
    menu.confirm("Confirm Load");
    menu.back_to_root();
}

fn assert_factory_patch_matches_expected_fixture(device: &DeviceDriver) {
    let scenario_payload = device.config_payload();
    let scenario_revision = scenario_payload["revision"].as_u64().unwrap();
    let expected_payload = expected_factory_patch_payload(scenario_revision);
    if scenario_payload != expected_payload {
        panic!(
            "factory patch scenario payload does not match expected user-flow fixture\nscenario:\n{}\nexpected:\n{}",
            serde_json::to_string_pretty(&scenario_payload).unwrap(),
            serde_json::to_string_pretty(&expected_payload).unwrap()
        );
    }
}

fn expected_factory_patch_payload(revision: u64) -> Value {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.clear_patch_state().unwrap();
    let mut expected = runner.config_payload();
    expected["revision"] = revision.into();
    merge_fixture(&mut expected, expected_factory_patch_fixture());
    expected
}

fn expected_factory_patch_fixture() -> Value {
    json!({
        "runtimeConfig": {
            "activeBehavior": "looper",
            "activeLayerIndex": 2,
            "auxBindings": {
                "aux1": {
                    "pressAction": null,
                    "turnKey": "instruments.1.sample.filter.cutoffHz"
                }
            },
            "instruments": [
                {
                    "type": "synth",
                    "name": "Synth",
                    "mixer": { "route": "fx_bus_1" },
                    "synth": { "filter": { "cutoffHz": 9921 } }
                },
                {
                    "type": "sampler",
                    "name": "Sampler",
                    "sample": {
                        "selectedSlot": 3,
                        "slots": [
                            { "path": "samples/Drum/kick/Kick2.wav" },
                            { "path": "samples/Drum/claps/distkit-clap.wav" },
                            {},
                            { "path": "samples/Drum/hihat open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav" }
                        ],
                        "assignments": [
                            { "level": null, "sampleSlot": 0, "x": 0, "y": 0 },
                            { "level": null, "sampleSlot": 0, "x": 1, "y": 0 },
                            { "level": null, "sampleSlot": 0, "x": 2, "y": 0 },
                            { "level": null, "sampleSlot": 0, "x": 3, "y": 0 },
                            { "level": null, "sampleSlot": 0, "x": 4, "y": 0 },
                            { "level": null, "sampleSlot": 0, "x": 5, "y": 0 },
                            { "level": null, "sampleSlot": 0, "x": 6, "y": 0 },
                            { "level": null, "sampleSlot": 0, "x": 7, "y": 0 },
                            { "level": null, "sampleSlot": 1, "x": 0, "y": 1 },
                            { "level": null, "sampleSlot": 1, "x": 1, "y": 1 },
                            { "level": null, "sampleSlot": 1, "x": 2, "y": 1 },
                            { "level": null, "sampleSlot": 1, "x": 3, "y": 1 },
                            { "level": null, "sampleSlot": 1, "x": 4, "y": 1 },
                            { "level": null, "sampleSlot": 1, "x": 5, "y": 1 },
                            { "level": null, "sampleSlot": 1, "x": 6, "y": 1 },
                            { "level": null, "sampleSlot": 1, "x": 7, "y": 1 },
                            { "level": null, "sampleSlot": 3, "x": 0, "y": 2 },
                            { "level": null, "sampleSlot": 3, "x": 1, "y": 2 },
                            { "level": null, "sampleSlot": 3, "x": 2, "y": 2 },
                            { "level": null, "sampleSlot": 3, "x": 3, "y": 2 },
                            { "level": null, "sampleSlot": 3, "x": 4, "y": 2 },
                            { "level": null, "sampleSlot": 3, "x": 5, "y": 2 },
                            { "level": null, "sampleSlot": 3, "x": 6, "y": 2 },
                            { "level": null, "sampleSlot": 3, "x": 7, "y": 2 }
                        ]
                    }
                },
                {
                    "type": "synth",
                    "name": "Synth",
                    "noteBehavior": "hold",
                    "mixer": { "route": "fx_bus_1" }
                }
            ],
            "layers": [
                {
                    "name": "life",
                    "worlds": {
                        "behaviorId": "life",
                        "stepRate": "1/16",
                        "savedState": life_saved_state()
                    },
                    "pulses": { "pitch": { "startingNote": 62 } }
                },
                {
                    "name": "sequencer",
                    "worlds": {
                        "behaviorId": "sequencer",
                        "stepRate": "1/8",
                        "savedState": {
                            "cells": [
                                true, false, true, false, true, false, true, false,
                                false, false, true, false, true, false, false, false,
                                false, true, false, true, false, true, false, true,
                                false, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false
                            ],
                            "height": 8,
                            "width": 8
                        }
                    },
                    "pulses": {
                        "eventEnabled": true,
                        "scanAxis": "rows",
                        "scanMode": "scanning",
                        "scanUnit": "1/8"
                    }
                },
                {
                    "name": "looper",
                    "worlds": {
                        "behaviorId": "looper",
                        "stepRate": "1/8",
                        "savedState": {
                            "lengthSteps": 16,
                            "mode": "overdub",
                            "steps": [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []]
                        }
                    },
                    "pulses": {
                        "eventEnabled": true,
                        "pitch": { "startingNote": 62 }
                    }
                }
            ],
            "mixer": {
                "buses": [{
                    "name": "Delay+Duck",
                    "slot1": {
                        "type": "delay",
                        "params": {
                            "timeMode": "ms",
                            "timeNote": "1/8",
                            "timeMs": 250,
                            "feedback": 0.35,
                            "mixPct": 35,
                            "spreadPct": 0
                        }
                    },
                    "slot2": {
                        "type": "duck",
                        "params": {
                            "source": "I2",
                            "threshold": 0.08,
                            "amountPct": 60,
                            "attackMs": 8,
                            "releaseMs": 160
                        }
                    }
                }]
            },
            "sparksFx": {
                "assignments": [
                    { "x": 0, "y": 0, "config": stutter_fx() },
                    { "x": 1, "y": 0, "config": freeze_fx() },
                    { "x": 2, "y": 0, "config": pitch_fx(0) },
                    { "x": 3, "y": 0, "config": pitch_fx(7) }
                ],
                "selected": pitch_fx(7)
            },
            "sparksMode": "fx",
            "xy": {
                "x": {
                    "invert": false,
                    "key": "instruments.0.synth.filter.cutoffHz",
                    "kind": "number",
                    "label": "Cutoff",
                    "max": 255.0,
                    "min": 0.0,
                    "step": 1.0
                },
                "y": {
                    "invert": false,
                    "key": "instruments.0.synth.filter.resonance",
                    "kind": "number",
                    "label": "Res",
                    "max": 255.0,
                    "min": 0.0,
                    "step": 1.0
                }
            }
        },
        "system": { "sparksMode": "fx" }
    })
}

fn merge_fixture(target: &mut Value, fixture: Value) {
    match (target, fixture) {
        (Value::Object(target), Value::Object(fixture)) => {
            for (key, value) in fixture {
                if let Some(existing) = target.get_mut(&key) {
                    merge_fixture(existing, value);
                } else {
                    target.insert(key, value);
                }
            }
        }
        (Value::Array(target), Value::Array(fixture)) => {
            for (index, value) in fixture.into_iter().enumerate() {
                if let Some(existing) = target.get_mut(index) {
                    merge_fixture(existing, value);
                } else {
                    target.push(value);
                }
            }
        }
        (target, fixture) => *target = fixture,
    }
}

fn life_saved_state() -> Value {
    json!({
        "cells": [
            false, true, false, true, true, false, false, false,
            false, false, true, true, true, false, false, false,
            true, true, true, true, true, false, false, false,
            true, true, true, false, false, true, true, true,
            true, true, true, false, false, true, true, true,
            false, false, false, true, false, true, true, false,
            false, false, false, true, true, false, false, false,
            false, false, false, true, true, false, false, false
        ],
        "gliderSpawnInterval": 0,
        "height": 8,
        "randomCellsPerTick": 0,
        "randomTickInterval": 1,
        "spawnStep": 0,
        "triggerTypes": vec!["none"; 64],
        "width": 8
    })
}

fn stutter_fx() -> Value {
    json!({
        "fxType": "stutter",
        "params": { "depthPct": 100, "rateHz": 8 },
        "targetKey": "master"
    })
}

fn freeze_fx() -> Value {
    json!({
        "fxType": "freeze",
        "params": { "mixPct": 100, "releaseMs": 500 },
        "targetKey": "master"
    })
}

fn pitch_fx(semitones: i32) -> Value {
    json!({
        "fxType": "pitch_shift",
        "params": { "cents": 0, "mixPct": 100, "semitones": semitones },
        "targetKey": "master"
    })
}
