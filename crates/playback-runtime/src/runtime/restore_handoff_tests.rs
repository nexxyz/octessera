use crate::tests::support::FakeHost;
use crate::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig,
    PlaybackRuntime, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand, RuntimeConfig,
    RuntimeErrorDomain, RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeRecovery, RuntimeStoreResult, RuntimeUserDataRestorePhase, RuntimeUserDataRestoreStatus,
};
use serde_json::{json, Value};

#[derive(Default)]
pub(super) struct RestoreHost {
    pub(super) inner: FakeHost,
    system_reply: Option<RuntimeStoreResult>,
    patch_reply: Option<RuntimeStoreResult>,
    requests: Vec<RuntimePlatformRequest>,
    pub(super) acknowledge_count: usize,
    call_order: Vec<&'static str>,
}

impl HostAdapter for RestoreHost {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        self.inner.handle_musical_event(event)
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        self.requests.push(request.clone());
        let reply = match request.operation() {
            RuntimeOperation::StoreLoadSystem => {
                self.call_order.push("system-load");
                self.system_reply.take()
            }
            RuntimeOperation::StoreLoadDefault => {
                self.call_order.push("patch-load");
                self.patch_reply.take()
            }
            _ => {
                if matches!(request.effect, RuntimePlatformEffect::MidiPanic) {
                    self.call_order.push("midi-panic");
                }
                return self.inner.handle_platform_effect(request);
            }
        };
        self.inner.effects.push(request.effect.clone());
        Ok(reply
            .map(|result| HostMessage::RuntimeResult {
                result: result.with_identity(request.request_id.clone(), request.revision),
            })
            .into_iter()
            .collect())
    }

    fn acknowledge_restored_state(&mut self) -> Result<(), RuntimeAdapterError> {
        self.acknowledge_count += 1;
        self.call_order.push("restore-ack");
        Ok(())
    }

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        self.inner.handle_audio_command(command)
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        self.inner.handle_midi_message(bytes)
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        self.inner.silence_internal_audio()
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.inner.panic_external_midi()
    }
}

pub(super) fn runtime_runner() -> (PlaybackRuntime, NativeRunner, RestoreHost) {
    let full: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(full).unwrap();
    runner.skip_startup_splash();
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = RestoreHost::default();
    runtime
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut host,
        )
        .unwrap();
    (runtime, runner, host)
}

pub(super) fn documents() -> (Value, Value) {
    let full: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let documents = crate::split_system_patch_documents(&full).unwrap();
    (documents.system, documents.patch)
}

fn begin_restore(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut RestoreHost,
) -> crate::RuntimeIngest {
    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::UserDataRestoreStatus {
                    status: RuntimeUserDataRestoreStatus {
                        phase: RuntimeUserDataRestorePhase::Succeeded,
                    },
                }
                .with_identity("archive-restore".into(), Some(41)),
            },
            runner,
            host,
        )
        .unwrap()
}

fn requests_for(host: &RestoreHost, operation: RuntimeOperation) -> Vec<&RuntimePlatformRequest> {
    host.requests
        .iter()
        .filter(|request| request.operation() == operation)
        .collect()
}

#[test]
fn matching_system_then_patch_restore_acknowledges_once_after_native_apply() {
    let (mut runtime, mut runner, mut host) = runtime_runner();
    let (mut system, mut patch) = documents();
    system["runtimeConfig"]["displayBrightness"] = json!(27);
    patch["runtimeConfig"]["transport"]["bpm"] = json!(133.0);
    patch["runtimeConfig"]["bpm"] = json!(133.0);
    patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
        json!("userdata/User Kit/restore.wav");
    host.system_reply = Some(RuntimeStoreResult::LoadSystemResult {
        payload: Some(system),
    });
    host.patch_reply = Some(RuntimeStoreResult::LoadDefaultResult {
        payload: Some(patch.clone()),
    });

    let output = begin_restore(&mut runtime, &mut runner, &mut host);

    assert_eq!(host.acknowledge_count, 1);
    let ack_index = host
        .call_order
        .iter()
        .position(|call| *call == "restore-ack")
        .unwrap();
    let panic_index = host
        .call_order
        .iter()
        .position(|call| *call == "midi-panic")
        .unwrap();
    assert!(ack_index < panic_index);
    assert_eq!(
        runtime.last_snapshot().unwrap()["settings"]["displayBrightness"],
        27
    );
    assert_eq!(runtime.config().bpm, 133.0);
    assert_eq!(
        runtime.last_status().unwrap().transport,
        crate::RuntimeTransportState::Stopped
    );
    assert!(host
        .inner
        .effects
        .iter()
        .any(|effect| matches!(effect, RuntimePlatformEffect::MidiPanic)));
    assert!(runtime.last_status().unwrap().error.is_none());
    assert!(output.messages.iter().any(|message| matches!(
        message,
        RunnerMessage::Snapshot { snapshot }
            if snapshot["display"]["title"] == "Restore complete"
    )));

    let patch_request = requests_for(&host, RuntimeOperation::StoreLoadDefault)[0].clone();
    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult {
                    payload: Some(patch.clone()),
                }
                .with_identity(patch_request.request_id, patch_request.revision),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_eq!(host.acknowledge_count, 1);

    let ordinary_request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadDefault,
        "ordinary-after-restore".into(),
        Some(76),
    );
    runner.register_platform_request(&ordinary_request);
    let mut ordinary_patch = patch;
    ordinary_patch["runtimeConfig"]["transport"]["bpm"] = json!(127.0);
    ordinary_patch["runtimeConfig"]["bpm"] = json!(127.0);
    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult {
                    payload: Some(ordinary_patch),
                }
                .with_identity(ordinary_request.request_id, ordinary_request.revision),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_eq!(runtime.config().bpm, 127.0);
    assert_eq!(host.acknowledge_count, 1);
}

#[test]
fn invalid_or_missing_system_fails_restore_without_a_patch_request_or_ack() {
    for system_reply in [
        RuntimeStoreResult::LoadSystemResult { payload: None },
        RuntimeStoreResult::LoadSystemResult {
            payload: Some(json!({ "kind": "invalid-system" })),
        },
    ] {
        let (mut runtime, mut runner, mut host) = runtime_runner();
        host.system_reply = Some(system_reply);
        begin_restore(&mut runtime, &mut runner, &mut host);

        assert_eq!(host.acknowledge_count, 0);
        assert_eq!(
            requests_for(&host, RuntimeOperation::StoreLoadDefault).len(),
            0
        );
        let error = runtime.last_status().unwrap().error.as_ref().unwrap();
        assert_eq!(error.domain, RuntimeErrorDomain::Storage);
        assert_eq!(error.operation, RuntimeOperation::StoreLoadSystem);
        assert_eq!(error.recovery, RuntimeRecovery::RetainLastGood);
        assert_eq!(
            runtime.last_status().unwrap().transport,
            crate::RuntimeTransportState::Stopped
        );
        assert_eq!(host.inner.silence_calls, 0);
    }
}

#[test]
fn rejected_or_unmatched_restore_patch_is_typed_nonfatal_and_never_acks() {
    let (system, valid_patch) = documents();
    let mut truncated = valid_patch.clone();
    truncated["runtimeConfig"]["instruments"]
        .as_array_mut()
        .unwrap()
        .pop();
    let mut conflict_system = system.clone();
    let mut conflicting_patch = valid_patch.clone();
    conflict_system["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"] = json!("displayBrightness");
    conflicting_patch["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"] =
        json!("displayBrightness");

    for patch_reply in [
        RuntimeStoreResult::LoadDefaultResult { payload: None },
        RuntimeStoreResult::LoadDefaultResult {
            payload: Some(json!({ "kind": "octessera.patch", "schemaVersion": 2 })),
        },
        RuntimeStoreResult::LoadDefaultResult {
            payload: Some(truncated),
        },
        RuntimeStoreResult::LoadDefaultResult {
            payload: Some(conflicting_patch),
        },
    ] {
        let (mut runtime, mut runner, mut host) = runtime_runner();
        let old_bpm = runtime.config().bpm;
        host.system_reply = Some(RuntimeStoreResult::LoadSystemResult {
            payload: Some(conflict_system.clone()),
        });
        host.patch_reply = Some(patch_reply);
        let output = begin_restore(&mut runtime, &mut runner, &mut host);

        assert_eq!(host.acknowledge_count, 0);
        assert_eq!(runtime.config().bpm, old_bpm);
        assert_eq!(host.inner.silence_calls, 0);
        let error = runtime.last_status().unwrap().error.as_ref().unwrap();
        assert_eq!(error.domain, RuntimeErrorDomain::Storage);
        assert_eq!(error.operation, RuntimeOperation::StoreLoadDefault);
        assert_eq!(error.recovery, RuntimeRecovery::RetainLastGood);
        let patch_request = requests_for(&host, RuntimeOperation::StoreLoadDefault)[0];
        assert_eq!(
            error.request_id.as_deref(),
            Some(patch_request.request_id.as_str())
        );
        assert_eq!(error.revision, patch_request.revision);
        assert_eq!(
            runtime.last_snapshot().unwrap()["display"]["title"],
            "Restore failed"
        );
        assert!(output.messages.iter().any(|message| matches!(
            message,
            RunnerMessage::Snapshot { snapshot }
                if snapshot["runtimeError"]["operation"] == "store_load_default"
        )));
        assert!(!runner
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult {
                    payload: Some(valid_patch.clone()),
                }
                .with_identity("wrong-child".into(), Some(1)),
            })
            .unwrap()
            .iter()
            .any(
                |message| matches!(message, RunnerMessage::PlatformEffects { effects }
                if effects.iter().any(|effect| matches!(effect,
                    RuntimePlatformEffect::StoreSaveDefault { .. })))
            ));
        assert_eq!(host.acknowledge_count, 0);
    }
}

#[test]
fn out_of_order_wrong_revision_and_unidentified_child_results_do_not_apply() {
    let (mut runtime, mut runner, mut host) = runtime_runner();
    let (_, patch) = documents();
    let old_bpm = runtime.config().bpm;
    begin_restore(&mut runtime, &mut runner, &mut host);

    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult {
                    payload: Some(patch.clone()),
                },
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_eq!(runtime.config().bpm, old_bpm);
    assert_eq!(host.acknowledge_count, 0);

    let system_request = requests_for(&host, RuntimeOperation::StoreLoadSystem)
        .first()
        .unwrap()
        .to_owned();
    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadSystemResult {
                    payload: Some(documents().0),
                }
                .with_identity(system_request.request_id.clone(), system_request.revision),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    let patch_request = requests_for(&host, RuntimeOperation::StoreLoadDefault)[0].clone();
    let before_bpm = runtime.config().bpm;
    for result in [
        RuntimeStoreResult::LoadDefaultResult {
            payload: Some(patch.clone()),
        }
        .with_identity("wrong-patch-id".into(), patch_request.revision),
        RuntimeStoreResult::LoadDefaultResult {
            payload: Some(patch.clone()),
        }
        .with_identity(patch_request.request_id.clone(), Some(999)),
    ] {
        runtime
            .dispatch_host_message(
                HostMessage::RuntimeResult { result },
                &mut runner,
                &mut host,
            )
            .unwrap();
        assert_eq!(runtime.config().bpm, before_bpm);
        assert_eq!(host.acknowledge_count, 0);
    }
    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult {
                    payload: Some(patch),
                },
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_eq!(runtime.config().bpm, before_bpm);
    assert_eq!(host.acknowledge_count, 0);
}
