use super::*;
use crate::RuntimeUiPulse;

#[test]
fn transport_and_event_indicators_appear_in_snapshot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let start = runner.send(HostMessage::MidiRealtimeStart).unwrap();
    assert!(start.iter().any(|message| matches!(
        message,
        RunnerMessage::UiPulse {
            pulse: RuntimeUiPulse::TransportFlash { flash, .. }
        } if flash == "measure"
    )));
    let start_snapshot = snapshot_from(&start);
    assert_eq!(start_snapshot["transportIcon"], "play");
    assert_eq!(start_snapshot["transportFlash"], "measure");
    assert_eq!(start_snapshot["cpuLoadRatio"], 0.0);

    let tick = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    assert!(tick.iter().any(|message| matches!(
        message,
        RunnerMessage::UiPulse {
            pulse: RuntimeUiPulse::TriggerPulse { .. }
        }
    )));
    assert!(tick.iter().any(|message| matches!(
        message,
        RunnerMessage::UiPulse {
            pulse: RuntimeUiPulse::TransportFlash { flash, .. }
        } if flash == "beat"
    )));
    let tick_snapshot = snapshot_from(&tick);
    assert_eq!(tick_snapshot["transportFlash"], "beat");
    assert_eq!(tick_snapshot["eventDotOn"], true);

    runner.transport = RuntimeTransportState::Paused;
    let paused_snapshot = runner.snapshot().unwrap();
    assert_eq!(paused_snapshot["transportIcon"], "pause");

    runner.transport = RuntimeTransportState::Stopped;
    let stopped_snapshot = runner.snapshot().unwrap();
    assert_eq!(stopped_snapshot["transportIcon"], "stop");
}

fn configured_scanning_sequencer_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scan_axis = "rows".into();
    runner.sense_parts[0].scan_unit = "1/16".into();
    runner.sense_parts[0].scanned_slot = 0;
    runner.sense_parts[0].scanned_action = "note_on".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    runner
}

#[test]
fn startup_playback_resets_scan_accumulators() {
    let mut runner = configured_scanning_sequencer_runner();
    runner.part_pulse_accumulators[0] = 5;
    runner.tick = 7;
    runner.current_ppqn_pulse = 42;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();

    assert_eq!(runner.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.tick, 0);
    assert_eq!(runner.current_ppqn_pulse, 0);
    assert_eq!(runner.part_pulse_accumulators[0], 0);
}

#[test]
fn stop_then_start_restarts_scanning_from_zero_accumulator() {
    let mut runner = configured_scanning_sequencer_runner();

    runner.transport = RuntimeTransportState::Playing;
    let _ = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 3,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(runner.part_pulse_accumulators[0] > 0);

    runner.ui.shift_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    runner.ui.shift_held = false;

    assert_eq!(runner.transport, RuntimeTransportState::Stopped);
    assert_eq!(runner.tick, 0);
    assert_eq!(runner.part_pulse_accumulators[0], 0);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();

    assert_eq!(runner.transport, RuntimeTransportState::Playing);
    assert_eq!(runner.tick, 0);
    assert_eq!(runner.current_ppqn_pulse, 0);
    assert_eq!(runner.part_pulse_accumulators[0], 0);
}

#[test]
fn stop_then_start_restarts_scanning_from_first_lane() {
    let mut runner = configured_scanning_sequencer_runner();
    runner.transport = RuntimeTransportState::Playing;
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    runner.ui.shift_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    runner.ui.shift_held = false;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();

    let snapshot = runner.snapshot().unwrap();
    let cells = snapshot["leds"]["cells"].as_array().unwrap();
    let first_lane = cells[display_index(0, 0)].as_object().unwrap();
    let second_lane = cells[display_index(0, 1)].as_object().unwrap();

    assert!(first_lane["r"].as_i64().unwrap() > second_lane["r"].as_i64().unwrap());
}

#[test]
fn part_two_scanning_uses_second_instrument_slot_without_bleeding_to_part_one() {
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
    runner.part_behavior_ids[1] = "sequencer".into();
    runner.sense_parts[1].scan_mode = "scanning".into();
    runner.sense_parts[1].scan_axis = "rows".into();
    runner.sense_parts[1].scan_unit = "1/16".into();
    runner.sense_parts[1].scanned_slot = 1;
    runner.sense_parts[1].scanned_action = "note_on".into();
    runner.select_active_part(1).unwrap();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
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
fn inactive_scanning_part_uses_its_sampler_slot_after_config_load() {
    let payload = json!({
        "runtimeConfig": {
            "activePartIndex": 0,
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
            "parts": [
                {
                    "l1": { "behaviorId": "life", "behaviorConfig": {}, "saveGridState": false },
                    "l2": {
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
                    "l1": {
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
                    "l2": {
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
fn numeric_string_slot_payloads_load_with_legacy_one_based_compatibility() {
    let payload = json!({
        "runtimeConfig": {
            "parts": [{
                "l2": {
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

    assert_eq!(runner.sense_parts[0].activate_slot, 0);
    assert_eq!(runner.sense_parts[0].stable_slot, 1);
    assert_eq!(runner.sense_parts[0].deactivate_slot, 0);
}

#[test]
fn fn_space_preserves_sequencer_cells() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.sense_parts[0].scan_mode = "scanning".into();
    runner.sense_parts[0].scan_axis = "rows".into();
    runner.sense_parts[0].scan_unit = "1/16".into();
    runner.sense_parts[0].scanned_slot = 0;
    runner.sense_parts[0].scanned_action = "note_on".into();
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    runner
        .engine
        .set_interpretation_profile(runner.interpretation_profile.clone());
    runner.transport = RuntimeTransportState::Playing;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
        })
        .unwrap();
    assert!(runner.engine.model().unwrap().cells[platform_core::grid_index(0, 0)]);

    runner.ui.fn_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "zero");
    assert!(runner.engine.model().unwrap().cells[platform_core::grid_index(0, 0)]);

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "full");
    assert!(runner.engine.model().unwrap().cells[platform_core::grid_index(0, 0)]);
}

#[test]
fn fn_space_restores_triggered_input_events_after_reenable() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();

    let before = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
        })
        .unwrap();
    assert!(before.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, MusicalEvent::NoteOn { .. }))
    )));

    runner.ui.fn_held = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "zero");

    let muted = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 3, "y": 3 }),
        })
        .unwrap();
    assert!(!muted.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, MusicalEvent::NoteOn { .. }))
    )));

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
        })
        .unwrap();
    assert_eq!(runner.trigger_gate_modes[0], "full");

    let restored = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 4, "y": 3 }),
        })
        .unwrap();
    assert!(restored.iter().any(|message| matches!(
        message,
        RunnerMessage::MusicalEvents { events }
            if events.iter().any(|event| matches!(event, MusicalEvent::NoteOn { .. }))
    )));
}
