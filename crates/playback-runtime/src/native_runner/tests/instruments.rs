use super::*;

#[test]
fn synth_preset_load_changes_full_synth_payload_and_filter_resonance_is_editable() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let initial = runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"].clone();

    runner.load_synth_preset(0, "bright_pluck");
    let changed = runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"].clone();

    assert_ne!(changed, initial);
    assert_eq!(changed["filter"]["resonance"], 34);
    assert_eq!(changed["filter"]["type"], "lowpass");
    assert_eq!(changed["filter"]["cutoffHz"], 7200);
    assert_eq!(changed["osc1"]["waveform"], "saw");
    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.synth.osc1.waveform"),
        Some("saw".into())
    );
    assert_eq!(
        runner.menu.value_for_key("instruments.0.synth.filter.type"),
        Some("lowpass".into())
    );

    runner.menu.state.stack = vec![2, 0, 0, 2, 1];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 1;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 2;
    runner.menu.turn(5);
    runner.apply_menu_state().unwrap();

    runner.menu.state.cursor = 3;
    runner.menu.turn(10);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 3];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(1);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 3];
    runner.menu.state.cursor = 1;
    runner.menu.state.editing = true;
    runner.menu.turn(-2);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 3];
    runner.menu.state.cursor = 2;
    runner.menu.state.editing = true;
    runner.menu.turn(5);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 4];
    runner.menu.state.cursor = 1;
    runner.menu.state.editing = true;
    runner.menu.turn(-20);
    runner.apply_menu_state().unwrap();

    runner.menu.state.stack = vec![2, 0, 0, 2, 5];
    runner.menu.state.cursor = 0;
    runner.menu.state.editing = true;
    runner.menu.turn(5);
    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["waveform"],
        "square"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["octave"],
        1
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["levelPct"],
        91
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["osc1"]["detuneCents"],
        10
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["filter"]["type"],
        "highpass"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["filter"]["cutoffHz"],
        6969
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["filter"]["resonance"],
        39
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["amp"]
            ["velocitySensitivityPct"],
        80
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["synth"]["ampEnv"]["attackMs"],
        28
    );
}

#[test]
fn sample_slot_menu_is_one_based_but_payload_remains_zero_based_and_back_exits_assign() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.menu.rebuild(runner.menu_config());

    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.sample.selectedSlot")
            .unwrap(),
        "1"
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"]["selectedSlot"],
        0
    );

    runner.sample_assign = Some((0, 0));
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();
    assert!(runner.sample_assign.is_none());
}

#[test]
fn legacy_bus_route_normalizes_on_config_load_and_persists_canonical_route() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["mixer"]["route"] = json!("bus_2");

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.instruments[0].route, "fx_bus_2");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["mixer"]["route"],
        "fx_bus_2"
    );
}

#[test]
fn legacy_trigger_gates_migrate_to_probability_map() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["parts"][0]["l1"]["triggerGates"] = Value::Array(
        (0..GRID_WIDTH * GRID_HEIGHT)
            .map(|index| Value::Bool(index != 3))
            .collect(),
    );
    payload["runtimeConfig"]["parts"][0]["l2"]
        .as_object_mut()
        .unwrap()
        .remove("triggerProbabilityMap");

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.trigger_probability_maps[0][3], "zero");
    assert_eq!(runner.trigger_probability_maps[0][4], "full");
}

#[test]
fn legacy_eight_position_pan_payload_scales_to_native_pan_range() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["panPositions"] = json!(8);
    payload["runtimeConfig"]["instruments"][0]["mixer"]["panPos"] = json!(7);
    payload["runtimeConfig"]["instruments"][1]["mixer"]["panPos"] = json!(3);

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.instruments[0].pan_pos, PAN_POSITION_COUNT - 1);
    assert_eq!(runner.instruments[1].pan_pos, PAN_POSITION_COUNT / 2);
}

#[test]
fn sample_slots_and_assignments_are_sanitized_on_load() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["sample"]["selectedSlot"] = json!(99);
    payload["runtimeConfig"]["instruments"][0]["sample"]["slots"] = Value::Array(
        (0..12)
            .map(|index| json!({ "path": format!("s{index}.wav") }))
            .collect(),
    );
    payload["runtimeConfig"]["instruments"][0]["sample"]["assignments"] = json!([
        { "x": 99, "y": 99, "sampleSlot": 99, "level": "loud" }
    ]);

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.instruments[0].selected_sample_slot, 7);
    assert_eq!(
        runner.instruments[0].sample_paths[7].as_deref(),
        Some("s7.wav")
    );
    assert_eq!(runner.instruments[0].sample_assignments[0].x, 7);
    assert_eq!(runner.instruments[0].sample_assignments[0].y, 7);
    assert_eq!(runner.instruments[0].sample_assignments[0].sample_slot, 7);
    assert_eq!(runner.instruments[0].sample_assignments[0].level, None);
}

#[test]
fn dance_fx_payload_sanitizes_type_target_and_params() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_touch_fx_payload(&json!({
        "selected": { "fxType": "stutter", "targetKey": "bad", "params": { "rateHz": 99, "depthPct": -5, "ignored": 42 } },
        "assignments": [
            { "x": 1, "y": 2, "config": { "fxType": "pitch_shift", "targetKey": "instrument_8", "params": { "semitones": 99, "cents": -200, "mixPct": 250 } } }
        ]
    }));

    assert_eq!(runner.dance_fx_selected["targetKey"], "master");
    assert_eq!(runner.dance_fx_selected["params"]["rateHz"], 32);
    assert_eq!(runner.dance_fx_selected["params"]["depthPct"], 0);
    assert!(runner.dance_fx_selected["params"].get("ignored").is_none());
    assert_eq!(
        runner.dance_fx_assignments[0].config["params"]["semitones"],
        24
    );
    assert_eq!(
        runner.dance_fx_assignments[0].config["params"]["cents"],
        -100
    );
    assert_eq!(
        runner.dance_fx_assignments[0].config["params"]["mixPct"],
        100
    );
}

#[test]
fn sample_assign_mode_supports_shift_row_and_fn_shift_column() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.sample_assign = Some((0, 2));

    runner.ui.shift_held = true;
    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(runner.instruments[0].sample_assignments.len(), GRID_WIDTH);
    assert!(runner.instruments[0]
        .sample_assignments
        .iter()
        .all(|assignment| assignment.y == 3 && assignment.sample_slot == 2));

    runner.instruments[0].sample_assignments.clear();
    runner.ui.shift_held = false;
    runner.ui.combined_modifier_held = true;
    runner.handle_sample_assignment_grid_press(4, 1);
    assert_eq!(runner.instruments[0].sample_assignments.len(), GRID_HEIGHT);
    assert!(runner.instruments[0]
        .sample_assignments
        .iter()
        .all(|assignment| assignment.x == 4 && assignment.sample_slot == 2));
}

#[test]
fn sample_assignment_cycles_velocity_levels_when_enabled() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_velocity_levels_enabled = true;
    runner.sample_assign = Some((0, 2));

    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(
        runner.instruments[0].sample_assignments[0].level.as_deref(),
        Some("high")
    );
    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(
        runner.instruments[0].sample_assignments[0].level.as_deref(),
        Some("medium")
    );
    runner.handle_sample_assignment_grid_press(1, 3);
    assert_eq!(
        runner.instruments[0].sample_assignments[0].level.as_deref(),
        Some("low")
    );
    runner.handle_sample_assignment_grid_press(1, 3);
    assert!(runner.instruments[0].sample_assignments.is_empty());
}

#[test]
fn sample_assignment_velocity_level_uses_configured_values() {
    let mut instrument = NativeInstrumentSlot::new(0);
    instrument.sample_base_velocity = 80;
    instrument.sample_velocity_high = 127;
    instrument.sample_velocity_medium = 64;
    instrument.sample_velocity_low = 32;
    let base = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: None,
    };
    let high = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: Some("high".into()),
    };
    let medium = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: Some("medium".into()),
    };
    let low = NativeSampleAssignment {
        x: 0,
        y: 0,
        sample_slot: 0,
        level: Some("low".into()),
    };

    assert_eq!(sampler_assignment_velocity(127, &base, &instrument), 80);
    assert_eq!(sampler_assignment_velocity(127, &high, &instrument), 127);
    assert_eq!(sampler_assignment_velocity(127, &medium, &instrument), 64);
    assert_eq!(sampler_assignment_velocity(127, &low, &instrument), 32);
}

#[test]
fn sense_velocity_and_filter_lanes_modulate_mapped_events() {
    let sense = NativeSensePart {
        x_velocity: NativeValueLane {
            enabled: true,
            from: 10,
            to: 110,
            grid_offset: 0,
            curve: "linear".into(),
        },
        y_filter_cutoff: NativeValueLane {
            enabled: true,
            from: 20,
            to: 120,
            grid_offset: 0,
            curve: "linear".into(),
        },
        ..NativeSensePart::default()
    };
    let events = vec![MusicalEvent::NoteOn {
        channel: 2,
        note: 60,
        velocity: 100,
        duration_ms: Some(150),
    }];
    let intents = vec![CellTriggerIntent {
        x: 7,
        y: 7,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    }];

    let out = apply_sampler_assignments_for_instruments(events, &intents, 0, &[], Some(&sense));

    assert!(matches!(
        out.as_slice(),
        [
            MusicalEvent::Cc {
                channel: 2,
                controller: 74,
                value: 120
            },
            MusicalEvent::NoteOn { velocity: 110, .. }
        ]
    ));
}

#[test]
fn midi_instrument_channel_remaps_note_and_cc_events() {
    let mut instrument = NativeInstrumentSlot {
        kind: "midi".into(),
        midi_channel: 10,
        ..NativeInstrumentSlot::new(0)
    };
    instrument.midi_enabled = true;
    let sense = NativeSensePart {
        y_filter_cutoff: NativeValueLane {
            enabled: true,
            from: 20,
            to: 120,
            grid_offset: 0,
            curve: "linear".into(),
        },
        ..NativeSensePart::default()
    };
    let intent = CellTriggerIntent {
        x: 0,
        y: 7,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };

    let out = apply_sampler_assignments_for_instruments(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(120),
        }],
        &[intent],
        0,
        &[instrument],
        Some(&sense),
    );

    assert!(matches!(
        out.as_slice(),
        [
            MusicalEvent::Cc { channel: 9, .. },
            MusicalEvent::NoteOn { channel: 9, .. }
        ]
    ));
}

#[test]
fn sampler_assignment_remaps_note_off_and_suppresses_unmapped_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_assignments = vec![NativeSampleAssignment {
        x: 2,
        y: 3,
        sample_slot: 4,
        level: None,
    }];
    let instruments = vec![runner.instruments[0].clone()];
    let assigned_intent = CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::CellTriggerKind::Deactivate,
    };
    let unmapped_intent = CellTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };

    let assigned = apply_sampler_assignments_for_instruments(
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }],
        &[assigned_intent],
        0,
        &instruments,
        None,
    );
    let unmapped = apply_sampler_assignments_for_instruments(
        vec![
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
                duration_ms: Some(120),
            },
            MusicalEvent::NoteOff {
                channel: 0,
                note: 60,
            },
        ],
        &[unmapped_intent.clone(), unmapped_intent],
        0,
        &instruments,
        None,
    );

    assert!(matches!(
        assigned.as_slice(),
        [MusicalEvent::NoteOff {
            channel: 0,
            note: 40
        }]
    ));
    assert!(unmapped.is_empty());
}
