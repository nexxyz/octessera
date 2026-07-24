use super::*;

#[test]
pub(crate) fn inactive_layer_encoder_config_edit_rebuilds_only_target_layer() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.select_layer_behavior(1, "brain").unwrap();
    let initial_seed_cells = runner.layer_engines[1]
        .as_ref()
        .unwrap()
        .serialized_state()
        .unwrap()["randomSeedCells"]
        .as_i64()
        .unwrap();
    let active_before = runner.engine.serialized_state().unwrap();
    let key = "layers.1.worlds.behaviorConfig.randomSeedCells";
    assert!(runner.menu.focus_item_key(key));
    runner.menu.state.editing = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.behavior.id(), "life");
    assert_eq!(runner.engine.serialized_state().unwrap(), active_before);
    assert_eq!(
        runner.layer_behavior_configs[1]["randomSeedCells"],
        initial_seed_cells + 1
    );
    assert_eq!(
        runner.layer_engines[1]
            .as_ref()
            .unwrap()
            .serialized_state()
            .unwrap()["randomSeedCells"],
        initial_seed_cells + 1
    );
}

#[test]
pub(crate) fn inactive_layer_aux_config_edit_changes_execution_state() {
    let mut runner = brain_target_runner();
    let initial_seed_cells = runner.layer_engines[1]
        .as_ref()
        .unwrap()
        .serialized_state()
        .unwrap()["randomSeedCells"]
        .as_i64()
        .unwrap();
    let active_before = runner.engine.serialized_state().unwrap();
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("layers.1.worlds.behaviorConfig.randomSeedCells".into()),
        press_action: None,
    });

    runner.handle_aux_turn(0, 1).unwrap();

    assert_eq!(runner.engine.serialized_state().unwrap(), active_before);
    assert_eq!(
        runner.layer_behavior_configs[1]["randomSeedCells"],
        initial_seed_cells + 1
    );
    assert!(runner.config_dirty);
}

#[test]
pub(crate) fn inactive_layer_xy_modulation_changes_execution_state() {
    let mut runner = brain_target_runner();
    runner.active_sparks_mode = "xy".into();
    runner.xy_y_binding = Some(NativeParamBinding {
        key: "layers.1.worlds.behaviorConfig.randomSeedCells".into(),
        label: Some("Spawn Count".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 3, "y": 7 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.layer_behavior_configs[1]["randomSeedCells"], 20);
    assert_eq!(
        runner.layer_engines[1]
            .as_ref()
            .unwrap()
            .serialized_state()
            .unwrap()["randomSeedCells"],
        20
    );
}

#[test]
pub(crate) fn patch_config_is_authoritative_over_stale_and_explicit_saved_state_is_preserved() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        behavior_config: json!({ "randomCellsPerTick": 1, "randomTickInterval": 1 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner
        .engine
        .on_input(
            platform_core::DeviceInput::BehaviorAction(platform_core::BehaviorActionInput {
                action_type: "spawnGlider".into(),
            }),
            runner.transport.bpm as f32,
        )
        .unwrap();
    let saved_state = runner.engine.serialized_state().unwrap();
    assert!(saved_state["cells"]
        .as_array()
        .unwrap()
        .iter()
        .any(|cell| cell.as_bool() == Some(true)));

    runner
        .apply_patch_payload_preserving_device(json!({
            "kind": "octessera.patch",
            "schemaVersion": 1,
            "runtimeConfig": {
                "layers": [{
                    "worlds": {
                        "behaviorConfig": { "randomCellsPerTick": 2 }
                    }
                }]
            }
        }))
        .unwrap();
    let expected_reset_state = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        behavior_config: json!({ "randomCellsPerTick": 2, "randomTickInterval": 1 }),
        ..NativeRunnerConfig::default()
    })
    .unwrap()
    .engine
    .serialized_state()
    .unwrap();
    assert_eq!(runner.behavior_config["randomCellsPerTick"], 2);
    assert_eq!(
        runner.engine.serialized_state().unwrap(),
        expected_reset_state
    );

    runner
        .apply_patch_payload_preserving_device(json!({
            "kind": "octessera.patch",
            "schemaVersion": 1,
            "runtimeConfig": {
                "layers": [{
                    "worlds": {
                        "behaviorId": "life",
                        "behaviorConfig": { "randomCellsPerTick": 3 },
                        "savedState": saved_state
                    }
                }]
            }
        }))
        .unwrap();
    assert_eq!(runner.behavior_config["randomCellsPerTick"], 3);
    assert!(runner.engine.serialized_state().unwrap()["cells"]
        .as_array()
        .unwrap()
        .iter()
        .any(|cell| cell.as_bool() == Some(true)));
    assert_eq!(
        runner.engine.serialized_state().unwrap()["randomCellsPerTick"],
        3
    );
}

#[test]
pub(crate) fn behavior_config_history_survives_none_transitions_per_layer_and_behavior() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.select_layer_behavior(1, "life").unwrap();
    runner
        .apply_layer_behavior_config_deltas(1, &[("randomCellsPerTick".into(), json!(7))])
        .unwrap();
    runner.select_layer_behavior(1, "brain").unwrap();
    runner
        .apply_layer_behavior_config_deltas(1, &[("randomSeedCells".into(), json!(9))])
        .unwrap();
    runner.select_layer_behavior(1, "none").unwrap();
    runner.select_layer_behavior(1, "life").unwrap();
    assert_eq!(runner.layer_behavior_configs[1]["randomCellsPerTick"], 7);
    runner.select_layer_behavior(1, "none").unwrap();
    runner.select_layer_behavior(1, "brain").unwrap();
    assert_eq!(runner.layer_behavior_configs[1]["randomSeedCells"], 9);
}

#[test]
pub(crate) fn behavior_config_history_round_trips_through_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.select_layer_behavior(1, "life").unwrap();
    runner
        .apply_layer_behavior_config_deltas(1, &[("randomCellsPerTick".into(), json!(7))])
        .unwrap();
    runner.select_layer_behavior(1, "brain").unwrap();
    runner
        .apply_layer_behavior_config_deltas(1, &[("randomSeedCells".into(), json!(9))])
        .unwrap();

    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["layers"][1]["worlds"]["behaviorConfigHistory"]["life"]
            ["randomCellsPerTick"],
        7
    );

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    restored.select_layer_behavior(1, "none").unwrap();
    restored.select_layer_behavior(1, "life").unwrap();
    assert_eq!(restored.layer_behavior_configs[1]["randomCellsPerTick"], 7);
    restored.select_layer_behavior(1, "none").unwrap();
    restored.select_layer_behavior(1, "brain").unwrap();
    assert_eq!(restored.layer_behavior_configs[1]["randomSeedCells"], 9);
}

#[test]
pub(crate) fn reset_behavior_reseeds_the_target_engine_and_marks_dirty() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let expected_state = runner.engine.serialized_state().unwrap();
    runner
        .engine
        .on_input(
            platform_core::DeviceInput::BehaviorAction(platform_core::BehaviorActionInput {
                action_type: "spawnGlider".into(),
            }),
            runner.transport.bpm as f32,
        )
        .unwrap();
    assert!(runner.engine.serialized_state().unwrap()["cells"]
        .as_array()
        .unwrap()
        .iter()
        .any(|cell| cell.as_bool() == Some(true)));
    runner.config_dirty = false;
    runner
        .execute_menu_action(crate::native_menu::NativeMenuAction::ResetBehavior)
        .unwrap();

    assert_eq!(runner.engine.serialized_state().unwrap(), expected_state);
    assert!(runner.config_dirty);
}

#[test]
pub(crate) fn engine_replacement_releases_held_audio_and_midi_notes() {
    assert_replacement_releases_held_note(false);
    assert_replacement_releases_held_note(true);
}

fn assert_replacement_releases_held_note(midi: bool) {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.instruments[0].note_behavior = "hold".into();
    if midi {
        runner.instruments[0].kind = "midi".into();
        runner.instruments[0].midi_enabled = true;
        runner.instruments[0].midi_channel = 3;
    }
    runner.sync_engine_runtime_config();
    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(press.iter().any(|message| match message {
        RunnerMessage::MusicalEvents { events } if !midi => events.iter().any(|event| {
            matches!(
                event,
                platform_core::MusicalEvent::NoteOn {
                    duration_ms: None,
                    ..
                }
            )
        }),
        RunnerMessage::MidiEvents { events } if midi => events.iter().any(|event| {
            matches!(
                event,
                platform_core::MusicalEvent::NoteOn {
                    duration_ms: None,
                    ..
                }
            )
        }),
        _ => false,
    }));
    runner.delayed_link_events[0].push(DelayedRoutedEvents {
        remaining_steps: 1,
        events: RoutedMusicalEvents::default(),
    });
    runner.link_arp_held_notes[0].push(LinkArpHeldNote {
        audio: !midi,
        channel: 0,
        note: 60,
        velocity: 100,
    });

    runner
        .apply_layer_behavior_config_deltas(0, &[("quantize".into(), json!("step"))])
        .unwrap();
    let replacement = runner.messages_with_snapshot().unwrap();
    assert!(replacement.iter().any(|message| match message {
        RunnerMessage::MusicalEvents { events } if !midi => events
            .iter()
            .any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. })),
        RunnerMessage::MidiEvents { events } if midi => events
            .iter()
            .any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. })),
        _ => false,
    }));
    assert!(runner.delayed_link_events[0].is_empty());
    assert!(runner.link_arp_held_notes[0].is_empty());
}

fn brain_target_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.select_layer_behavior(1, "brain").unwrap();
    runner
}

#[test]
pub(crate) fn fast_layer_edit_clears_only_the_target_link_arp_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.link_arp_held_notes[0].push(LinkArpHeldNote {
        audio: true,
        channel: 0,
        note: 60,
        velocity: 100,
    });
    runner.link_arp_held_notes[1].push(LinkArpHeldNote {
        audio: true,
        channel: 0,
        note: 64,
        velocity: 100,
    });
    let key = "layers.0.worlds.behaviorConfig.randomCellsPerTick";
    assert!(runner.menu.focus_item_key(key));
    runner.menu.state.editing = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(runner.link_arp_held_notes[0].is_empty());
    assert_eq!(runner.link_arp_held_notes[1].len(), 1);
}

#[test]
pub(crate) fn batched_target_deltas_compare_only_the_final_value() {
    let mut runner = brain_target_runner();
    runner.layer_behavior_configs[1] = json!({ "randomSeedCells": 4 });
    runner.config_dirty = false;
    runner.pending.pending_autosave_payload_due_at = None;
    runner.behavior_state_serialization_calls.set(0);
    runner.layer_behavior_rebuilds = 0;
    runner.fast_autosave_marks = 0;
    runner.link_arp_held_notes[1].push(LinkArpHeldNote {
        audio: true,
        channel: 0,
        note: 60,
        velocity: 100,
    });

    let before_engine = runner.layer_engines[1].as_ref().unwrap().state().clone();

    assert!(!runner
        .apply_layer_behavior_config_deltas(
            1,
            &[
                ("randomSeedCells".into(), json!(5)),
                ("randomSeedCells".into(), json!(4)),
            ],
        )
        .unwrap());

    assert_eq!(runner.layer_behavior_configs[1]["randomSeedCells"], 4);
    assert_eq!(
        runner.layer_engines[1].as_ref().unwrap().state().clone(),
        before_engine
    );
    assert_eq!(runner.link_arp_held_notes[1].len(), 1);
    assert!(!runner.config_dirty);
    assert!(runner.pending.pending_autosave_payload_due_at.is_none());
    assert_eq!(runner.behavior_state_serialization_calls.get(), 0);
    assert_eq!(runner.layer_behavior_rebuilds, 0);
    assert_eq!(runner.fast_autosave_marks, 0);
}

#[test]
pub(crate) fn fast_behavior_edit_reports_replacement_errors_without_committing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.layer_behavior_ids[0] = "unsupported-behavior".into();
    let before = runner.layer_behavior_configs[0].clone();
    let key = "layers.0.worlds.behaviorConfig.randomCellsPerTick";
    assert!(runner.menu.focus_item_key(key));
    runner.menu.state.editing = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.layer_behavior_configs[0], before);
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("unsupported native behavior `unsupported-behavior`")
    );
}
