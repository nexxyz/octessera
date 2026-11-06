use super::*;

fn device_input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn bind_to_aux1(runner: &mut NativeRunner, key: &str) -> Vec<RunnerMessage> {
    assert!(runner.menu.focus_item_key(key));
    let _ = device_input(runner, json!({ "type": "button_fn", "pressed": true }));
    let bound = device_input(runner, json!({ "type": "encoder_press", "id": "aux1" }));
    let _ = device_input(runner, json!({ "type": "button_fn", "pressed": false }));
    bound
}

fn aux1_turn_key(runner: &NativeRunner) -> Option<&str> {
    runner.aux_bindings[0]
        .as_ref()
        .and_then(|binding| binding.turn_key.as_deref())
}

fn saved_documents(runner: &NativeRunner) -> SystemPatchDocuments {
    split_local_system_patch_documents(&runner.config_payload()).unwrap()
}

fn load_patch(runner: &mut NativeRunner, patch: &Value) {
    runner
        .apply_store_result(RuntimeStoreResult::LoadDefaultResult {
            payload: Some(patch.clone()),
        })
        .unwrap();
}

#[test]
fn aux_bound_to_master_volume_saves_with_the_patch_and_reloads() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let bound = bind_to_aux1(&mut runner, "masterVolume");

    assert!(!bound.iter().any(|message| matches!(message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect,
                RuntimePlatformEffect::StoreSaveSystem { .. }))
    )));
    assert!(runner.config_dirty);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    assert_eq!(runner.pending.system_persistence.dirty_revision, None);
    assert_ne!(
        runner.reboot_blocked_by_pending_saves(),
        Some("Save or apply System settings before reboot")
    );

    let documents = saved_documents(&runner);
    assert_eq!(
        documents.patch["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "masterVolume"
    );
    assert!(documents.system["runtimeConfig"]
        .get("auxBindings")
        .is_none());

    let mut reloaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    load_patch(&mut reloaded, &documents.patch);
    assert_eq!(aux1_turn_key(&reloaded), Some("masterVolume"));
}

#[test]
fn loading_another_patch_brings_its_own_aux_layout() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    bind_to_aux1(&mut runner, "masterVolume");
    let master_patch = saved_documents(&runner).patch;
    let mut other = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    bind_to_aux1(&mut other, "transport.bpm");
    let bpm_patch = saved_documents(&other).patch;

    load_patch(&mut runner, &bpm_patch);
    assert_eq!(aux1_turn_key(&runner), Some("transport.bpm"));
    load_patch(&mut runner, &master_patch);
    assert_eq!(aux1_turn_key(&runner), Some("masterVolume"));
}

#[test]
fn turning_an_aux_bound_to_master_volume_changes_master_volume() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    bind_to_aux1(&mut runner, "masterVolume");
    let start = runner.display.ui.master_volume;

    let turned = device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux1", "delta": -1 }),
    );

    assert_eq!(runner.display.ui.master_volume, start - 5);
    assert!(turned.iter().any(|message| matches!(message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(command,
                RuntimeAudioCommand::SetMasterVolume { .. }))
    )));
}
