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

fn start_deferred_write(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut ReplayHost,
) -> RuntimePlatformRequest {
    assert!(runner.menu.focus_item_key("transport.bpm"));
    let _ = press(runtime, runner, host);
    let _ = turn(runtime, runner, host, 1);
    runner.make_deferred_menu_apply_due_for_test();
    let messages = runner.flush_deferred_menu_apply().unwrap();
    runtime
        .dispatch_runner_messages(messages, runner, host)
        .unwrap();
    host.requests
        .iter()
        .find(|request| request.operation() == RuntimeOperation::StoreSaveDefault)
        .cloned()
        .expect("deferred default write")
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

fn identified_save(request: &RuntimePlatformRequest, ok: bool) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SaveDefaultResult { ok, is_auto: None }),
            request_id: request.request_id.clone(),
            revision: request.revision,
        },
    }
}

#[test]
fn autosave_restart_intent_survives_an_older_default_write() {
    for deferred in [false, true] {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.auto_save_default = true;
        runner.menu.rebuild(runner.menu_config());
        let mut host = ReplayHost::default();
        let older = if deferred {
            start_deferred_write(&mut runtime, &mut runner, &mut host)
        } else {
            start_ordinary_write(&mut runtime, &mut runner, &mut host)
        };

        edit_buffer_and_commit(&mut runtime, &mut runner, &mut host);
        assert!(
            runner.display.confirm_dialog.is_none(),
            "auto save opened a manual choice for {deferred:?}"
        );

        let _ = runtime
            .dispatch_host_message(identified_save(&older, true), &mut runner, &mut host)
            .unwrap();
        let restart_requests: Vec<_> = host
            .requests
            .iter()
            .filter(|request| {
                matches!(
                    request.effect,
                    RuntimePlatformEffect::StoreSaveDefault {
                        mode: Some(ref mode),
                        ..
                    } if mode == "restart-everything"
                )
            })
            .collect();
        assert_eq!(restart_requests.len(), 1, "{deferred:?}");
        let restart_request = restart_requests[0];
        assert_eq!(
            runner
                .display
                .confirm_dialog
                .as_ref()
                .map(|dialog| dialog.title.as_str()),
            Some("Saving...")
        );
        assert_eq!(
            restart_request.effect,
            RuntimePlatformEffect::StoreSaveDefault {
                payload: runner.config_payload(),
                mode: Some("restart-everything".into()),
            }
        );

        let output = runtime
            .dispatch_host_message(
                identified_save(restart_request, true),
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
                .filter(|request| request.operation() == RuntimeOperation::StoreSaveDefault)
                .count(),
            2,
            "{deferred:?}"
        );
    }
}

#[test]
fn immediate_autosave_failure_rearms_the_normal_deferred_retry() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.menu.rebuild(runner.menu_config());
    let mut host = ReplayHost {
        fail_first_default_save: true,
        ..ReplayHost::default()
    };

    assert!(runner.menu.focus_item_key("transport.bpm"));
    let _ = press(&mut runtime, &mut runner, &mut host);
    let _ = turn(&mut runtime, &mut runner, &mut host, 1);
    runner.make_deferred_menu_apply_due_for_test();
    let messages = runner.flush_deferred_menu_apply().unwrap();
    runtime
        .dispatch_runner_messages(messages, &mut runner, &mut host)
        .unwrap();

    assert_eq!(host.default_save_calls, 1);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());

    runner.make_deferred_menu_apply_due_for_test();
    let retry = runner.flush_deferred_menu_apply().unwrap();
    assert_eq!(
        retry
            .iter()
            .filter(|message| matches!(
                message,
                RunnerMessage::PlatformEffects { effects }
                    if effects.iter().any(|effect| matches!(
                        effect,
                        RuntimePlatformEffect::StoreSaveDefault {
                            mode: Some(mode),
                            ..
                        } if mode == "deferred"
                    ))
            ))
            .count(),
        1
    );
    runtime
        .dispatch_runner_messages(retry, &mut runner, &mut host)
        .unwrap();
    assert_eq!(host.default_save_calls, 2);
}

#[test]
fn default_load_is_blocked_during_default_write_and_abandons_loaded_ownership() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let old_revision = {
        assert!(runner.menu.focus_item_key("default.save"));
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
        runner.restart_settings.pending_write_revision().unwrap()
    };
    runner.register_default_write_request("old-default", Some(old_revision));

    assert!(runner.menu.focus_item_key("default.load"));
    let _ = direct_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = direct_input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let messages = direct_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreLoadDefault))
    )));
    assert!(runner.restart_settings.has_pending_write());

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let payload = loaded.config_payload();
    assert!(loaded.menu.focus_item_key("default.save"));
    let _ = direct_input(
        &mut loaded,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = direct_input(
        &mut loaded,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let _ = direct_input(
        &mut loaded,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(loaded.restart_settings.has_pending_write());
    let _ = loaded
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            },
        })
        .unwrap();
    assert!(!loaded.restart_settings.has_pending_write());
    assert!(loaded.pending.pending_save_revision.is_none());

    assert!(loaded.menu.focus_item_key("default.save"));
    let _ = direct_input(
        &mut loaded,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let _ = direct_input(
        &mut loaded,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    let messages = direct_input(
        &mut loaded,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { mode: None, .. }
            ))
    )));

    assert_eq!(
        old_revision,
        runner.restart_settings.pending_write_revision().unwrap()
    );
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
    let _ = direct_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    let revision = runner.restart_settings.pending_write_revision().unwrap();
    runner.register_default_write_request("setting-only", Some(revision));

    let result = || HostMessage::RuntimeResult {
        result: RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: None,
            }),
            request_id: "setting-only".into(),
            revision: Some(revision),
        },
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
