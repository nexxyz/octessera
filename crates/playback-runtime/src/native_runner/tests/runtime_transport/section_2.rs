use super::*;

#[test]
pub(crate) fn layer_two_scanning_uses_second_instrument_slot_without_bleeding_to_layer_one() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.instruments[1].kind = "sampler".into();
    runner.instruments[1].sample_assignments = vec![NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 4,
        level: None,
    }];
    runner.layer_behavior_ids[1] = "sequencer".into();
    runner.pulses_layers[1].scan_mode = "scanning".into();
    runner.pulses_layers[1].scan_axis = "rows".into();
    runner.pulses_layers[1].scan_unit = "1/16".into();
    runner.pulses_layers[1].scanned_slot = 1;
    runner.pulses_layers[1].scanned_action = "note_on".into();
    runner.select_active_layer(1).unwrap();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    let notes = musical_note_ons(&messages);
    assert!(notes
        .iter()
        .any(|(channel, note)| *channel == 1 && *note == 40));
    assert!(!notes.iter().any(|(channel, _)| *channel == 0));
}

#[test]
pub(crate) fn triplet_scan_unit_advances_at_expected_pulse_count() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.pulses_layers[0].scan_mode = "scanning".into();
    runner.pulses_layers[0].scan_axis = "rows".into();
    runner.pulses_layers[0].scan_unit = "1/8T".into();
    runner.pulses_layers[0].scanned_action = "note_on".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();

    let before_unit = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 7,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(musical_note_ons(&before_unit).is_empty());
    assert_eq!(runner.layer_pulse_accumulators[0], 7);

    let on_unit = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(!musical_note_ons(&on_unit).is_empty());
    assert_eq!(runner.layer_pulse_accumulators[0], 0);
}

#[test]
pub(crate) fn changing_layer_four_behavior_does_not_reset_layer_two_playback_phase() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.layer_behavior_ids[1] = "sequencer".into();
    runner.pulses_layers[1].scan_mode = "scanning".into();
    runner.pulses_layers[1].scan_axis = "rows".into();
    runner.pulses_layers[1].scan_unit = "1/16".into();
    runner.pulses_layers[1].scanned_slot = 1;
    runner.pulses_layers[1].scanned_action = "note_on".into();
    runner.select_active_layer(1).unwrap();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 3,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(runner.layer_pulse_accumulators[1], 3);
    let ppqn_before = runner.current_ppqn_pulse;

    runner.select_active_layer(3).unwrap();
    runner
        .rebuild_engine(platform_core::get_native_behavior("sequencer").unwrap())
        .unwrap();
    runner
        .rebuild_engine(platform_core::get_native_behavior("none").unwrap())
        .unwrap();

    assert_eq!(runner.current_ppqn_pulse, ppqn_before);
    assert_eq!(runner.layer_pulse_accumulators[1], 3);
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 3,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(runner.layer_pulse_accumulators[1], 0);
}

#[test]
pub(crate) fn switching_to_scanning_layer_restores_its_visual_scan_tick() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner.layer_behavior_ids[1] = "sequencer".into();
    runner.pulses_layers[1].scan_mode = "scanning".into();
    runner.pulses_layers[1].scan_axis = "columns".into();
    runner.pulses_layers[1].scan_unit = "1/16".into();

    runner.select_active_layer(1).unwrap();
    runner.select_active_layer(0).unwrap();
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 12,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_eq!(runner.layer_ticks[1], 2);

    runner.tick = 99;
    runner.select_active_layer(1).unwrap();
    assert_eq!(runner.tick, 2);
}

#[test]
pub(crate) fn inactive_scanning_layer_uses_its_sampler_slot_after_config_load() {
    let payload = json!({
        "runtimeConfig": {
            "activeLayerIndex": 0,
            "instruments": [
                { "type": "synth", "name": "synth", "autoName": true },
                {
                    "type": "sampler",
                    "name": "sampler",
                    "autoName": true,
                    "sample": {
                        "assignments": [{ "x": 0, "y": 0, "sampleSlot": 4, "level": null }],
                        "slots": []
                    }
                }
            ],
            "layers": [
                {
                    "worlds": { "behaviorId": "life", "behaviorConfig": {}, "saveGridState": false },
                    "pulses": {
                        "eventEnabled": true,
                        "stateNotesEnabled": false,
                        "scanMode": "immediate",
                        "mapping": {
                            "activate": { "slot": 0, "action": "note_on" },
                            "stable": { "slot": "none", "action": "none" },
                            "deactivate": { "slot": 0, "action": "note_off" },
                            "scanned": { "slot": "none", "action": "none" },
                            "scanned_empty": { "slot": "none", "action": "none" }
                        }
                    }
                },
                {
                    "worlds": {
                        "behaviorId": "sequencer",
                        "behaviorConfig": {},
                        "saveGridState": true,
                        "behaviorState": {
                            "width": 8,
                            "height": 8,
                            "cells": [
                                true, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false,
                                false, false, false, false, false, false, false, false
                            ]
                        },
                        "stepRate": "1/16"
                    },
                    "pulses": {
                        "eventEnabled": false,
                        "stateNotesEnabled": true,
                        "scanMode": "scanning",
                        "scanAxis": "rows",
                        "scanDirection": "forward",
                        "scanSections": 1,
                        "scanUnit": "1/16",
                        "mapping": {
                            "activate": { "slot": 0, "action": "note_on" },
                            "stable": { "slot": 0, "action": "none" },
                            "deactivate": { "slot": 0, "action": "note_off" },
                            "scanned": { "slot": 1, "action": "note_on" },
                            "scanned_empty": { "slot": "none", "action": "none" }
                        }
                    }
                }
            ]
        },
        "mappingConfig": platform_core::default_mapping_config()
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            },
        })
        .unwrap();
    runner.transport = RuntimeTransportState::Playing;

    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    let notes = musical_note_ons(&messages);
    assert!(notes
        .iter()
        .any(|(channel, note)| *channel == 1 && *note == 40));
    assert!(!notes.iter().any(|(channel, _)| *channel == 0));
}

#[test]
pub(crate) fn numeric_string_slot_payloads_load_with_legacy_one_based_compatibility() {
    let payload = json!({
        "runtimeConfig": {
            "layers": [{
                "pulses": {
                    "mapping": {
                        "activate": { "slot": "1", "action": "note_on" },
                        "stable": { "slot": "2", "action": "note_on" },
                        "deactivate": { "slot": "0", "action": "note_off" },
                        "scanned": { "slot": "none", "action": "none" },
                        "scanned_empty": { "slot": "none", "action": "none" }
                    }
                }
            }]
        }
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.pulses_layers[0].activate_slot, 0);
    assert_eq!(runner.pulses_layers[0].stable_slot, 1);
    assert_eq!(runner.pulses_layers[0].deactivate_slot, 0);
}
