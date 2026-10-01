use super::*;

fn system_document(runner: &NativeRunner) -> Value {
    split_system_patch_documents(&runner.config_payload())
        .unwrap()
        .system
}

fn assert_system_rejected_without_mutation(
    runner: &mut NativeRunner,
    mut system: Value,
    mutate: fn(&mut Value),
    label: &str,
) {
    let before = runner.config_payload();
    let transport = runner.transport.clone();
    let pending_revision = runner.pending.pending_save_revision;
    let autosave_due_at = runner.pending.pending_autosave_payload_due_at;
    let revision_state = (
        runner.config_revision,
        runner.dirty_revision,
        runner.config_dirty,
    );
    mutate(&mut system);
    let _ = runner.outbox.drain_audio_commands();
    let _ = runner.outbox.drain_platform_effects();
    assert!(
        runner
            .apply_system_document_preserving_patch(&system)
            .is_err(),
        "{label}"
    );
    assert_eq!(runner.config_payload(), before, "{label}");
    assert_eq!(runner.transport, transport, "{label}");
    assert_eq!(
        runner.pending.pending_save_revision, pending_revision,
        "{label}"
    );
    assert_eq!(
        runner.pending.pending_autosave_payload_due_at, autosave_due_at,
        "{label}"
    );
    assert_eq!(
        (
            runner.config_revision,
            runner.dirty_revision,
            runner.config_dirty
        ),
        revision_state,
        "{label}"
    );
    assert!(runner.outbox.drain_audio_commands().is_empty(), "{label}");
    assert!(!runner.outbox.has_platform_effects(), "{label}");
}

#[test]
fn system_document_changes_device_state_without_disturbing_live_patch_runtime() {
    for transport_state in [
        RuntimeTransportState::Stopped,
        RuntimeTransportState::Playing,
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig {
            behavior_id: "keys".into(),
            ..NativeRunnerConfig::default()
        })
        .unwrap();
        runner.instruments[0].note_behavior = "hold".into();
        runner.sync_engine_runtime_config();
        runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
                request_snapshot: None,
            })
            .unwrap();
        runner.transport.transport = transport_state;
        runner.pending.pending_save_revision = Some(19);
        runner.config_revision = 7;
        runner.dirty_revision = Some(23);
        runner.config_dirty = true;
        runner.mark_fast_autosave_dirty();
        assert!(runner.menu.focus_item_key("ghostCells"));

        let patch = runner.patch_payload().unwrap();
        let transport = runner.transport.clone();
        let menu_stack = runner.menu.state.stack.clone();
        let menu_cursor = runner.menu.state.cursor;
        let restart_settings = runner.restart_settings.clone();
        let revision_state = (
            runner.config_revision,
            runner.dirty_revision,
            runner.config_dirty,
            runner.pending.pending_save_revision,
            runner.pending.pending_autosave_payload_due_at,
        );
        let mut system = system_document(&runner);
        system["runtimeConfig"]["displayBrightness"] = json!(31);
        system["runtimeConfig"]["ghostCells"] = json!(true);

        runner
            .apply_system_document_preserving_patch(&system)
            .unwrap();

        assert_eq!(runner.display.ui.display_brightness, 31);
        assert!(runner.display.ui.ghost_cells);
        assert_eq!(runner.menu.number_for_key("displayBrightness"), Some(31));
        assert_eq!(
            runner.menu.value_for_key("ghostCells").as_deref(),
            Some("true")
        );
        assert_eq!(runner.menu.state.stack, menu_stack);
        assert_eq!(runner.menu.state.cursor, menu_cursor);
        assert_eq!(runner.restart_settings, restart_settings);
        assert_eq!(runner.patch_payload().unwrap(), patch);
        assert_eq!(runner.transport, transport);
        assert_eq!(
            (
                runner.config_revision,
                runner.dirty_revision,
                runner.config_dirty,
                runner.pending.pending_save_revision,
                runner.pending.pending_autosave_payload_due_at,
            ),
            revision_state
        );
        assert_eq!(runner.engine.drain_held_notes(usize::MAX).len(), 1);
        assert!(!runner
            .outbox
            .drain_platform_effects()
            .iter()
            .any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic)));
    }
}

#[test]
fn system_audio_changes_queue_existing_config_commands_without_resetting_live_patch() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.instruments[0].note_behavior = "hold".into();
    runner.sync_engine_runtime_config();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    let _ = runner.outbox.drain_audio_commands();
    runner.mark_fast_autosave_dirty();
    runner.pending.pending_save_revision = Some(31);
    runner.config_revision = 7;
    runner.dirty_revision = Some(23);
    runner.config_dirty = true;

    let patch = runner.patch_payload().unwrap();
    let restart_settings = runner.restart_settings.clone();
    let audio_revision = runner.audio_config_revision;
    let mut system = system_document(&runner);
    system["runtimeConfig"]["masterVolume"] = json!(44);
    system["runtimeConfig"]["sound"]["audioOutputBufferFrames"] = json!(512);
    system["runtimeConfig"]["dsp"]["workerWarningThreshold"] = json!("90");

    runner
        .apply_system_document_preserving_patch(&system)
        .unwrap();

    let commands = runner.outbox.drain_audio_commands();
    assert!(matches!(
        commands.as_slice(),
        [
            RuntimeAudioCommand::SetAudioConfig { config, .. },
            RuntimeAudioCommand::SetDspConfig { config: dsp, .. }
        ] if config["masterVolume"] == 44
            && dsp.worker_warning_threshold == realtime_engine::synth::WorkerWarningThreshold::Percent90
    ));
    assert_eq!(runner.audio_config_revision, audio_revision + 1);
    assert_eq!(runner.patch_payload().unwrap(), patch);
    assert_eq!(runner.config_revision, 7);
    assert_eq!(runner.dirty_revision, Some(23));
    assert!(runner.config_dirty);
    assert_eq!(runner.pending.pending_save_revision, Some(31));
    assert_eq!(runner.restart_settings, restart_settings);
    assert_eq!(runner.engine.drain_held_notes(usize::MAX).len(), 1);
    assert!(!runner
        .outbox
        .drain_platform_effects()
        .iter()
        .any(|effect| matches!(
            effect,
            RuntimePlatformEffect::MidiPanic
                | RuntimePlatformEffect::ApplyDeviceConfigReboot { .. }
                | RuntimePlatformEffect::Reboot
        )));
}

#[test]
fn invalid_system_device_values_are_rejected_before_mutation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.mark_fast_autosave_dirty();

    let invalid_dsp = system_document(&runner);
    assert_system_rejected_without_mutation(
        &mut runner,
        invalid_dsp,
        |system| system["runtimeConfig"]["dsp"] = json!({ "invalid": true }),
        "DSP",
    );
    let invalid_restart_setting = system_document(&runner);
    assert_system_rejected_without_mutation(
        &mut runner,
        invalid_restart_setting,
        |system| system["runtimeConfig"]["sound"]["audioOutputBufferFrames"] = json!(u64::MAX),
        "restart-sensitive",
    );
    let unavailable_optimization = system_document(&runner);
    assert_system_rejected_without_mutation(
        &mut runner,
        unavailable_optimization,
        |system| system["runtimeConfig"]["sound"]["optimizeFor"] = json!("capacity"),
        "unavailable optimization capacity",
    );
}

#[test]
fn jack_required_output_rejects_system_document_before_mutation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.mark_fast_autosave_dirty();
    let mut system = system_document(&runner);
    system["runtimeConfig"]["audioOutputs"] = json!({
        "dac": false,
        "usb": true,
        "hdmi": false
    });

    assert_system_rejected_without_mutation(&mut runner, system, |_| {}, "Jack-required output");
}

#[test]
fn conflicting_aux_system_claim_is_rejected_without_mutation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut full = runner.config_payload();
    full["runtimeConfig"]["shiftAuxBindings"]["aux2"]["turnKey"] = json!("sound.noteLengthMs");
    let documents = split_system_patch_documents(&full).unwrap();
    runner
        .apply_patch_payload_preserving_device(documents.patch)
        .unwrap();
    let _ = runner.outbox.drain_audio_commands();
    let _ = runner.outbox.drain_platform_effects();
    let mut conflicting_system = documents.system;
    conflicting_system["runtimeConfig"]["shiftAuxBindings"]["aux2"]["turnKey"] =
        json!("displayBrightness");
    let before = runner.config_payload();
    let transport = runner.transport.clone();
    let pending_revision = runner.pending.pending_save_revision;
    let autosave_due_at = runner.pending.pending_autosave_payload_due_at;
    let audio_revision = runner.audio_config_revision;
    let restart_settings = runner.restart_settings.clone();
    let menu_stack = runner.menu.state.stack.clone();
    let menu_cursor = runner.menu.state.cursor;
    let dirty = (
        runner.config_revision,
        runner.dirty_revision,
        runner.config_dirty,
    );

    assert!(runner
        .apply_system_document_preserving_patch(&conflicting_system)
        .is_err());

    assert_eq!(runner.config_payload(), before);
    assert_eq!(runner.transport, transport);
    assert_eq!(runner.pending.pending_save_revision, pending_revision);
    assert_eq!(
        runner.pending.pending_autosave_payload_due_at,
        autosave_due_at
    );
    assert_eq!(runner.audio_config_revision, audio_revision);
    assert_eq!(runner.restart_settings, restart_settings);
    assert_eq!(runner.menu.state.stack, menu_stack);
    assert_eq!(runner.menu.state.cursor, menu_cursor);
    assert_eq!(
        (
            runner.config_revision,
            runner.dirty_revision,
            runner.config_dirty
        ),
        dirty
    );
    assert!(runner.outbox.drain_audio_commands().is_empty());
    assert!(!runner.outbox.has_platform_effects());
}

#[test]
fn system_aux_device_sides_merge_with_patch_sides() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut full = runner.config_payload();
    full["runtimeConfig"]["auxBindings"]["aux1"] = json!({
        "turnKey": "sound.noteLengthMs",
        "pressAction": { "kind": "platform_effect", "action": "midi.panic" }
    });
    full["runtimeConfig"]["shiftAuxBindings"]["aux2"] = json!({
        "turnKey": "gridBrightness",
        "pressAction": { "kind": "behavior_action", "actionType": "source.shift" }
    });
    let documents = split_system_patch_documents(&full).unwrap();
    let system = documents.system;
    let patch = documents.patch;
    let expected = compose_system_patch_documents(&system, &patch).unwrap();
    runner
        .apply_patch_payload_preserving_device(patch.clone())
        .unwrap();

    runner
        .apply_system_document_preserving_patch(&system)
        .unwrap();

    let actual = runner.config_payload();
    for bank in ["auxBindings", "shiftAuxBindings"] {
        assert_eq!(
            actual["runtimeConfig"][bank],
            expected["runtimeConfig"][bank]
        );
    }
    assert_eq!(runner.patch_payload().unwrap(), patch);
}
