use super::*;
use crate::{
    HostAdapter, PlaybackRuntime, RuntimeAdapterError, RuntimeAudioCommand, RuntimeIngest,
    RuntimeOperation, RuntimePlatformRequest,
};
use platform_core::MusicalEvent;

fn direct_input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn dispatch_input(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut ReplayHost,
    input: Value,
) -> RuntimeIngest {
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input,
                request_snapshot: None,
            },
            runner,
            host,
        )
        .unwrap()
}

fn press(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut ReplayHost,
) -> RuntimeIngest {
    dispatch_input(
        runtime,
        runner,
        host,
        json!({ "type": "encoder_press", "id": "main" }),
    )
}

fn turn(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut ReplayHost,
    delta: i32,
) -> RuntimeIngest {
    dispatch_input(
        runtime,
        runner,
        host,
        json!({ "type": "encoder_turn", "delta": delta, "id": "main" }),
    )
}

#[derive(Default)]
struct ReplayHost {
    requests: Vec<RuntimePlatformRequest>,
    default_save_calls: usize,
    fail_first_default_save: bool,
}

impl HostAdapter for ReplayHost {
    fn handle_musical_event(&mut self, _event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        self.requests.push(request.clone());
        if matches!(
            request.effect,
            RuntimePlatformEffect::StoreSaveDefault { .. }
        ) {
            self.default_save_calls += 1;
            if self.fail_first_default_save && self.default_save_calls == 1 {
                return Err(RuntimeAdapterError::from("default save failed immediately"));
            }
        }
        Ok(Vec::new())
    }

    fn handle_audio_command(
        &mut self,
        _command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
}

fn start_ordinary_write(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut ReplayHost,
) -> RuntimePlatformRequest {
    assert!(runner.menu.focus_item_key("default.save"));
    let _ = press(runtime, runner, host);
    let _ = turn(runtime, runner, host, 1);
    let _ = press(runtime, runner, host);
    host.requests
        .iter()
        .find(|request| request.operation() == RuntimeOperation::StoreSaveDefault)
        .cloned()
        .expect("ordinary default write")
}

fn edit_buffer_and_commit(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut ReplayHost,
) {
    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    let _ = press(runtime, runner, host);
    let _ = turn(runtime, runner, host, 1);
    let _ = press(runtime, runner, host);
}

fn identified_save_with_identity(request_id: &str, revision: Option<u64>, ok: bool) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SaveDefaultResult { ok, is_auto: None }),
            request_id: request_id.into(),
            revision,
        },
    }
}

fn identified_system_save(request: &RuntimePlatformRequest, ok: bool) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::SaveSystemResult { ok }
            .with_identity(request.request_id.clone(), request.revision),
    }
}

#[test]
fn restart_sensitive_system_apply_waits_for_patch_save_then_persists_system_only() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.menu.rebuild(runner.menu_config());
    let mut host = ReplayHost::default();
    let patch_request = start_ordinary_write(&mut runtime, &mut runner, &mut host);

    edit_buffer_and_commit(&mut runtime, &mut runner, &mut host);
    assert_eq!(
        runner
            .display
            .confirm_dialog
            .as_ref()
            .map(|dialog| dialog.title.as_str()),
        Some("Apply System")
    );
    assert!(!host
        .requests
        .iter()
        .any(|request| request.operation() == RuntimeOperation::StoreSaveSystem));

    let _ = runtime
        .dispatch_host_message(
            identified_save_with_identity(&patch_request.request_id, patch_request.revision, true),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_eq!(
        runner
            .display
            .confirm_dialog
            .as_ref()
            .map(|dialog| dialog.title.as_str()),
        Some("Apply System")
    );
    assert!(runner
        .restart_settings
        .save_choice_setting_payload()
        .is_some());
    let _ = turn(&mut runtime, &mut runner, &mut host, 1);
    let _ = press(&mut runtime, &mut runner, &mut host);
    let system_request = host
        .requests
        .iter()
        .find(|request| request.operation() == RuntimeOperation::StoreSaveSystem)
        .cloned()
        .expect("System Apply request");
    assert!(matches!(
        &system_request.effect,
        RuntimePlatformEffect::StoreSaveSystem { payload }
            if payload["kind"] == "octessera.system"
                && payload["runtimeConfig"]["sound"]["audioOutputBufferFrames"].is_number()
    ));
    assert_eq!(system_request.revision, None);

    let output = runtime
        .dispatch_host_message(
            identified_system_save(&system_request, true),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_eq!(
        snapshot_from(&output.messages)["display"]["title"],
        "Restart?"
    );
    assert_eq!(
        host.requests
            .iter()
            .filter(|request| request.operation() == RuntimeOperation::StoreSaveSystem)
            .count(),
        1
    );
    assert!(!host.requests.iter().any(|request| matches!(
        request.effect,
        RuntimePlatformEffect::Reboot | RuntimePlatformEffect::Shutdown
    )));
}

#[test]
fn duplicate_setting_only_success_does_not_reconcile_unrelated_dirty_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = false;
    assert!(runner.menu.focus_item_key("transport.bpm"));
    let _ = direct_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = direct_input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let _ = direct_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );

    assert!(runner.menu.focus_item_key("sound.audioOutputBufferFrames"));
    let _ = direct_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = direct_input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let _ = direct_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = direct_input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let applied = direct_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let effect = applied
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                matches!(effect, RuntimePlatformEffect::StoreSaveSystem { .. })
                    .then(|| effect.clone())
            }),
            _ => None,
        })
        .expect("System Apply save effect");
    runner.register_platform_request(&RuntimePlatformRequest::new(
        effect,
        "setting-only".into(),
        None,
    ));

    let result = || HostMessage::RuntimeResult {
        result: RuntimeStoreResult::SaveSystemResult { ok: true }
            .with_identity("setting-only".into(), None),
    };
    let _ = runner.send(result()).unwrap();
    assert!(runner.pending.pending_save_revision.is_none());
    assert!(runner.config_dirty);
    let _ = runner.send(result()).unwrap();
    assert!(runner.config_dirty);
    assert!(runner.dirty_revision.is_some());
}

fn assert_focused_sound_row_fits(runner: &mut NativeRunner, key: &str) {
    assert!(runner.menu.focus_item_key(key));
    assert_eq!(runner.menu.current_key(), Some(key));
    assert!(
        runner.menu.value_for_key(key).is_some() || runner.menu.number_for_key(key).is_some(),
        "missing focused value for {key}"
    );
    let snapshot = runner.snapshot().unwrap();
    let expected_title = match key {
        "masterVolume" => "/SYS/Audio",
        "sound.noteLengthMs" | "sound.velocityScalePct" | "sound.velocityCurve" => "/SYS/Notes",
        "sound.audioOutputBufferFrames" => "/SYS/Audio/Engine",
        _ => "/SYS/Audio",
    };
    assert_eq!(snapshot["display"]["title"], expected_title);
    for line in snapshot["display"]["lines"].as_array().unwrap() {
        assert!(
            line.as_str().unwrap().chars().count() <= 19,
            "{key}: {line}"
        );
    }
}

#[test]
fn every_sound_key_and_value_fits_for_both_capability_variants() {
    let mut standard = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for key in [
        "masterVolume",
        "sound.noteLengthMs",
        "sound.velocityScalePct",
        "sound.velocityCurve",
        "sound.voiceStealingMode",
        "sound.audioOutputBufferFrames",
    ] {
        assert_focused_sound_row_fits(&mut standard, key);
    }

    let mut capacity = NativeRunner::new(NativeRunnerConfig {
        audio_optimization_capacity_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    for key in [
        "masterVolume",
        "sound.noteLengthMs",
        "sound.velocityScalePct",
        "sound.velocityCurve",
        "sound.voiceStealingMode",
        "sound.optimizeFor",
    ] {
        assert_focused_sound_row_fits(&mut capacity, key);
    }
}
