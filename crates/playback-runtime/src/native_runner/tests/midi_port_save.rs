use super::device_config_apply::reload_system_payload;
use super::*;
use crate::RuntimePlatformRequest;

fn press(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap()
}

fn effects(messages: &[RunnerMessage]) -> Vec<RuntimePlatformEffect> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn pick_port(runner: &mut NativeRunner, group: &str, port: MidiPort) -> RuntimePlatformEffect {
    runner.menu.open_labels(&["System", "MIDI", "MIDI Host"]);
    runner.menu.select_label(group);
    let opened = effects(&press(runner));
    let result = match opened.as_slice() {
        [RuntimePlatformEffect::MidiListOutputsRequest] => {
            RuntimeStoreResult::MidiListOutputsResult {
                outputs: vec![port.clone()],
            }
        }
        [RuntimePlatformEffect::MidiListInputsRequest] => {
            RuntimeStoreResult::MidiListInputsResult {
                inputs: vec![port.clone()],
            }
        }
        other => panic!("{group} should request its port list, got {other:?}"),
    };
    let _ = runner.send(HostMessage::RuntimeResult { result }).unwrap();
    runner.menu.select_label(&port.name);
    let picked = effects(&press(runner));
    assert_eq!(picked.len(), 1, "one select effect for {group}");
    picked[0].clone()
}

fn connected(runner: &mut NativeRunner, out_id: Option<&str>, in_id: Option<&str>) -> Value {
    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiStatus {
                ok: true,
                message: None,
                selected_out_id: out_id.map(Into::into),
                selected_in_id: in_id.map(Into::into),
            },
        })
        .unwrap();
    let payload = effects(&messages)
        .into_iter()
        .find_map(|effect| match effect {
            RuntimePlatformEffect::StoreSaveSystem { payload } => Some(payload),
            _ => None,
        })
        .expect("a connected port choice saves System");
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreSaveSystem {
            payload: payload.clone(),
        },
        "midi-port-save".into(),
        None,
    );
    runner.register_platform_request(&request);
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(request.request_id, request.revision),
        })
        .unwrap();
    payload
}

fn midi_keys(document: &Value) -> Vec<String> {
    let mut keys = document["runtimeConfig"]["midi"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

#[test]
fn picked_midi_ports_save_system_without_a_save_action_and_reload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(!runner.auto_save_default);
    let baseline =
        crate::native_runner::system_persistence::SystemPersistenceState::system_document(&runner)
            .unwrap();
    let output = MidiPort {
        id: "out1".into(),
        name: "Output 1".into(),
    };
    let input = MidiPort {
        id: "in1".into(),
        name: "Input 1".into(),
    };

    assert_eq!(
        pick_port(&mut runner, "MIDI Out", output),
        RuntimePlatformEffect::MidiSelectOutput {
            id: Some("out1".into())
        }
    );
    let saved = connected(&mut runner, Some("out1"), None);
    assert_eq!(saved["runtimeConfig"]["midi"]["outId"], "out1");
    assert_eq!(
        pick_port(&mut runner, "MIDI In", input),
        RuntimePlatformEffect::MidiSelectInput {
            id: Some("in1".into())
        }
    );
    let saved = connected(&mut runner, Some("out1"), Some("in1"));
    assert_eq!(saved["runtimeConfig"]["midi"]["inId"], "in1");
    assert_eq!(midi_keys(&saved), midi_keys(&baseline));
    assert_eq!(runner.pending.system_persistence.dirty_revision, None);
    assert_eq!(runner.reboot_blocked_by_pending_saves(), None);

    let reloaded = reload_system_payload(saved);
    assert_eq!(reloaded.selected_midi_output_id.as_deref(), Some("out1"));
    assert_eq!(reloaded.selected_midi_input_id.as_deref(), Some("in1"));
}

#[test]
fn unchanged_midi_status_does_not_save_system() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiStatus {
                ok: true,
                message: Some("Panic sent".into()),
                selected_out_id: None,
                selected_in_id: None,
            },
        })
        .unwrap();
    assert!(!effects(&messages)
        .iter()
        .any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveSystem { .. })));
}

#[test]
fn failed_connect_of_a_saved_port_keeps_it_and_a_later_connect_reports_ok() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut document =
        crate::native_runner::system_persistence::SystemPersistenceState::system_document(&runner)
            .unwrap();
    document["runtimeConfig"]["midi"]["outId"] = json!("out1");
    document["runtimeConfig"]["midi"]["inId"] = json!("in1");
    let mut runner = reload_system_payload(document);

    for (ok, message, status) in [
        (
            false,
            Some("MIDI output connect failed"),
            "MIDI output connect failed",
        ),
        (true, None, "MIDI ok"),
    ] {
        let messages = runner
            .send(HostMessage::RuntimeResult {
                result: RuntimeStoreResult::MidiStatus {
                    ok,
                    message: message.map(Into::into),
                    selected_out_id: Some("out1".into()),
                    selected_in_id: Some("in1".into()),
                },
            })
            .unwrap();
        assert!(!effects(&messages)
            .iter()
            .any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveSystem { .. })));
        assert_eq!(runner.midi_status.as_deref(), Some(status));
        assert_eq!(runner.selected_midi_output_id.as_deref(), Some("out1"));
        assert_eq!(runner.selected_midi_input_id.as_deref(), Some("in1"));
        assert_eq!(runner.pending.system_persistence.dirty_revision, None);
    }
}
