use super::*;

#[test]
pub(crate) fn keyed_behavior_menu_leaf_recomposes_with_one_target_replacement() {
    let mut runner = brain_target_runner();
    let key = "layers.1.worlds.behaviorConfig.randomSeedCells";
    runner.active_sparks_mode = "xy".into();
    runner.xy_touch = NativeXyTouch {
        x: 1.0,
        y: 0.5,
        display_x: 1.0,
        display_y: 0.5,
        active: true,
    };
    runner.xy_x_binding = Some(NativeParamBinding {
        key: key.into(),
        label: Some("Seed Cells".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.refresh_xy_runtime_sources();
    runner.process_dirty_modulation_step(false).unwrap();
    assert_eq!(
        runner.layer_engines[1]
            .as_ref()
            .unwrap()
            .serialized_state()
            .unwrap()["randomSeedCells"],
        20
    );
    runner.layer_behavior_rebuilds = 0;

    assert!(runner.menu.focus_item_key(key));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.layer_behavior_rebuilds, 1);
    assert_eq!(
        runner.layer_engines[1]
            .as_ref()
            .unwrap()
            .serialized_state()
            .unwrap()["randomSeedCells"],
        20
    );
    let rebased_base = runner.menu.number_for_key(key).unwrap();
    runner.set_param_binding_target("xy:x", None);
    assert_eq!(
        runner.layer_engines[1]
            .as_ref()
            .unwrap()
            .serialized_state()
            .unwrap()["randomSeedCells"],
        rebased_base
    );
}

#[test]
pub(crate) fn aux_behavior_turn_recomposes_held_value_with_one_replacement() {
    let mut runner = brain_target_runner();
    let key = "layers.1.worlds.behaviorConfig.randomSeedCells";
    runner.active_sparks_mode = "xy".into();
    runner.xy_touch = NativeXyTouch {
        x: 0.5,
        y: 0.5,
        display_x: 1.0,
        display_y: 0.5,
        active: true,
    };
    runner.xy_x_binding = Some(NativeParamBinding {
        key: key.into(),
        label: Some("Seed Cells".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.refresh_xy_runtime_sources();
    runner.process_dirty_modulation_step(false).unwrap();
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some(key.into()),
        press_action: None,
    });
    runner.layer_behavior_rebuilds = 0;

    runner.handle_aux_turn(0, 1).unwrap();

    assert_eq!(runner.layer_behavior_rebuilds, 1);
    assert_eq!(runner.layer_behavior_configs[1]["randomSeedCells"], 10);
    assert_eq!(
        runner.layer_engines[1]
            .as_ref()
            .unwrap()
            .serialized_state()
            .unwrap()["randomSeedCells"],
        10
    );
}

#[test]
pub(crate) fn physical_aux_turn_updates_base_and_recomposes_only_its_held_target() {
    let mut runner = brain_target_runner();
    runner
        .messages_with_snapshot()
        .expect("initial runtime snapshot");
    runner.active_sparks_mode = "xy".into();
    runner.xy_touch = NativeXyTouch {
        x: 0.5,
        y: 0.5,
        display_x: 0.5,
        display_y: 0.5,
        active: true,
    };
    let key = "layers.1.worlds.behaviorConfig.randomSeedCells";
    runner.xy_x_binding = Some(NativeParamBinding {
        key: key.into(),
        label: Some("Seed Cells".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(20.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.refresh_xy_runtime_sources();
    runner.process_dirty_modulation_step(false).unwrap();
    assert_eq!(runner.layer_behavior_configs[1]["randomSeedCells"], 10);

    bind_aux_turn_physical(&mut runner, key, false);
    let base_before = runner.menu.number_for_key(key).unwrap();
    runner.config_dirty = false;
    runner.dirty_revision = None;
    runner.pending.pending_autosave_payload_due_at = None;
    runner.pending.pending_save_revision = None;
    runner.fast_autosave_marks = 0;
    runner.behavior_state_serialization_calls.set(0);
    runner.layer_behavior_rebuilds = 0;
    runner.auto_save_default = true;
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.tick = 17;
    runner.transport.current_ppqn_pulse = 42;
    let transport_before = runner.transport.clone();
    let unrelated_layer_before = runner.engine.state().clone();
    let menu_rebuilds_before = runner.menu.rebuild_count;

    let first = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.layer_behavior_rebuilds, 1);
    assert_eq!(runner.fast_autosave_marks, 1);
    assert_eq!(runner.layer_behavior_configs[1]["randomSeedCells"], 10);
    assert_eq!(runner.engine.state().clone(), unrelated_layer_before);
    assert_eq!(runner.transport, transport_before);
    assert_eq!(runner.menu.rebuild_count, menu_rebuilds_before);
    assert!(runner
        .modulation_process
        .has_source(crate::native_runner::modulation_source::ModulationSourceId::play_x()));
    assert_eq!(
        runner
            .modulation_process
            .base_discrete
            .get(key)
            .and_then(|(_, value)| value.as_i64()),
        Some(i64::from(base_before + 1))
    );
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    assert!(runner.pending.pending_save_revision.is_none());
    assert_eq!(runner.behavior_state_serialization_calls.get(), 0);
    assert!(first.iter().all(|message| !matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { .. }
            ))
    )));

    let second = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.layer_behavior_rebuilds, 2);
    assert_eq!(runner.fast_autosave_marks, 2);
    assert_eq!(runner.transport, transport_before);
    assert_eq!(runner.menu.rebuild_count, menu_rebuilds_before);
    assert_eq!(runner.behavior_state_serialization_calls.get(), 0);
    assert!(second.iter().all(|message| !matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { .. }
            ))
    )));

    runner.make_deferred_menu_apply_due_for_test();
    let flushed = runner.flush_deferred_menu_apply().unwrap();
    let saves = flushed
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter(),
            _ => [].iter(),
        })
        .filter_map(|effect| match effect {
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                Some((payload, mode.as_deref()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].1, Some("deferred"));
    assert_eq!(
        saves[0].0["runtimeConfig"]["layers"][1]["worlds"]["behaviorConfig"]["randomSeedCells"],
        base_before + 2
    );
    assert!(runner.behavior_state_serialization_calls.get() > 0);
}

#[test]
pub(crate) fn physical_aux_binding_selection_keeps_normal_and_shifted_turn_banks() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let normal_key = "layers.0.worlds.behaviorConfig.randomCellsPerTick";
    let shifted_key = "layers.0.worlds.behaviorConfig.randomTickInterval";

    bind_aux_turn_physical(&mut runner, normal_key, false);
    bind_aux_turn_physical(&mut runner, shifted_key, true);

    assert_eq!(
        runner.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some(normal_key)
    );
    assert_eq!(
        runner.shift_aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some(shifted_key)
    );
}

fn bind_aux_turn_physical(runner: &mut NativeRunner, key: &str, shifted: bool) {
    assert!(runner.menu.focus_item_key(key));
    if shifted {
        runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "button_shift", "pressed": true }),
                request_snapshot: None,
            })
            .unwrap();
    }
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux1" }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": false }),
            request_snapshot: None,
        })
        .unwrap();
    if shifted {
        runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "button_shift", "pressed": false }),
                request_snapshot: None,
            })
            .unwrap();
    }
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
