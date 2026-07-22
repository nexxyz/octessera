use crate::midi;
use crate::samples;
use playback_runtime::{
    HostMessage, MidiPort, RuntimePlatformRequest, RuntimeStoreResult, SampleEntry,
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Clone, Debug)]
pub(crate) struct DesktopPlatformServiceRequest {
    pub(crate) request: RuntimePlatformRequest,
    pub(crate) kind: DesktopPlatformServiceKind,
}

#[derive(Clone, Debug)]
pub(crate) enum DesktopPlatformServiceKind {
    SampleList {
        instrument_slot: usize,
        sample_slot: usize,
        dir: String,
    },
    MidiListOutputs,
    MidiListInputs,
}

impl DesktopPlatformServiceRequest {
    pub(crate) fn new(request: RuntimePlatformRequest, kind: DesktopPlatformServiceKind) -> Self {
        Self { request, kind }
    }
}

pub(crate) struct DesktopPlatformService {
    pub(crate) request_tx: Sender<DesktopPlatformServiceRequest>,
    pub(crate) result_rx: Receiver<Vec<HostMessage>>,
}

pub(crate) fn spawn_desktop_platform_service() -> DesktopPlatformService {
    let (request_tx, request_rx) = mpsc::channel::<DesktopPlatformServiceRequest>();
    let (result_tx, result_rx) = mpsc::channel::<Vec<HostMessage>>();

    thread::spawn(move || {
        let mut next_id = 0_u64;
        while let Ok(request) = request_rx.recv() {
            next_id = next_id.saturating_add(1);
            let messages = shape_service_result(request);
            if result_tx.send(messages).is_err() {
                eprintln!(
                    "desktop platform service result receiver closed at request {}",
                    next_id
                );
                break;
            }
        }
    });

    DesktopPlatformService {
        request_tx,
        result_rx,
    }
}

pub(crate) fn shape_service_result(request: DesktopPlatformServiceRequest) -> Vec<HostMessage> {
    let runtime_request = request.request;
    let messages = match request.kind {
        DesktopPlatformServiceKind::SampleList {
            instrument_slot,
            sample_slot,
            dir,
        } => shape_sample_list_result(instrument_slot, sample_slot, dir, samples::sample_list),
        DesktopPlatformServiceKind::MidiListOutputs => {
            shape_midi_outputs_result(midi::list_outputs)
        }
        DesktopPlatformServiceKind::MidiListInputs => shape_midi_inputs_result(midi::list_inputs),
    };
    identify_service_messages(messages, &runtime_request)
}

pub(crate) fn shape_service_unavailable_result(
    request: DesktopPlatformServiceRequest,
    message: String,
) -> Vec<HostMessage> {
    let runtime_request = request.request;
    let messages = match request.kind {
        DesktopPlatformServiceKind::SampleList {
            instrument_slot,
            sample_slot,
            dir,
        } => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SampleListError {
                instrument_slot,
                sample_slot,
                dir,
                message,
            },
        }],
        DesktopPlatformServiceKind::MidiListOutputs
        | DesktopPlatformServiceKind::MidiListInputs => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::StoreError { message },
        }],
    };
    identify_service_messages(messages, &runtime_request)
}

fn identify_service_messages(
    messages: Vec<HostMessage>,
    request: &RuntimePlatformRequest,
) -> Vec<HostMessage> {
    messages
        .into_iter()
        .map(|message| match message {
            HostMessage::RuntimeResult { result } => {
                let result = match result {
                    RuntimeStoreResult::StoreError { message } => {
                        RuntimeStoreResult::RuntimeFailure {
                            error: request.failure_facts(message),
                        }
                    }
                    result => result,
                };
                HostMessage::RuntimeResult {
                    result: result.with_identity(request.request_id.clone(), request.revision),
                }
            }
            other => other,
        })
        .collect()
}

fn shape_sample_list_result(
    instrument_slot: usize,
    sample_slot: usize,
    dir: String,
    list: impl FnOnce(String) -> Result<Vec<samples::SampleEntry>, String>,
) -> Vec<HostMessage> {
    match list(dir.clone()) {
        Ok(entries) => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SampleListResult {
                instrument_slot,
                sample_slot,
                dir,
                entries: sample_entries(entries),
            },
        }],
        Err(message) => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SampleListError {
                instrument_slot,
                sample_slot,
                dir,
                message,
            },
        }],
    }
}

fn shape_midi_outputs_result(
    list: impl FnOnce() -> Result<Vec<midi::MidiPortInfo>, String>,
) -> Vec<HostMessage> {
    match list() {
        Ok(outputs) => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListOutputsResult {
                outputs: midi_ports(outputs),
            },
        }],
        Err(message) => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::StoreError { message },
        }],
    }
}

fn shape_midi_inputs_result(
    list: impl FnOnce() -> Result<Vec<midi::MidiPortInfo>, String>,
) -> Vec<HostMessage> {
    match list() {
        Ok(inputs) => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListInputsResult {
                inputs: midi_ports(inputs),
            },
        }],
        Err(message) => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::StoreError { message },
        }],
    }
}

fn midi_ports(ports: Vec<midi::MidiPortInfo>) -> Vec<MidiPort> {
    ports
        .into_iter()
        .map(|port| MidiPort {
            id: port.id,
            name: port.name,
        })
        .collect()
}

fn sample_entries(entries: Vec<samples::SampleEntry>) -> Vec<SampleEntry> {
    entries
        .into_iter()
        .map(|entry| SampleEntry {
            name: entry.name,
            path: entry.path,
            is_dir: entry.is_dir,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only_result(messages: Vec<HostMessage>) -> RuntimeStoreResult {
        assert_eq!(messages.len(), 1);
        match messages.into_iter().next().unwrap() {
            HostMessage::RuntimeResult { result } => match result {
                RuntimeStoreResult::Identified { result, .. } => *result,
                result => result,
            },
            _ => panic!("expected one runtime result"),
        }
    }

    #[test]
    fn sample_list_error_shapes_runtime_error() {
        let result = only_result(shape_sample_list_result(1, 2, "bad".into(), |_| {
            Err("nope".into())
        }));
        assert!(
            matches!(result, RuntimeStoreResult::SampleListError { instrument_slot: 1, sample_slot: 2, dir, message } if dir == "bad" && message == "nope")
        );
    }

    #[test]
    fn midi_output_error_returns_only_store_error() {
        let result = only_result(shape_midi_outputs_result(|| Err("midi unavailable".into())));
        assert!(
            matches!(result, RuntimeStoreResult::StoreError { message } if message == "midi unavailable")
        );
    }

    #[test]
    fn midi_input_error_returns_only_store_error() {
        let result = only_result(shape_midi_inputs_result(|| Err("midi unavailable".into())));
        assert!(
            matches!(result, RuntimeStoreResult::StoreError { message } if message == "midi unavailable")
        );
    }

    #[test]
    fn midi_empty_lists_remain_successful_results() {
        let outputs = only_result(shape_midi_outputs_result(|| Ok(Vec::new())));
        assert!(
            matches!(outputs, RuntimeStoreResult::MidiListOutputsResult { outputs } if outputs.is_empty())
        );

        let inputs = only_result(shape_midi_inputs_result(|| Ok(Vec::new())));
        assert!(
            matches!(inputs, RuntimeStoreResult::MidiListInputsResult { inputs } if inputs.is_empty())
        );
    }

    #[test]
    fn service_unavailable_midi_requests_return_only_store_error() {
        let outputs = only_result(shape_service_unavailable_result(
            DesktopPlatformServiceRequest::new(
                RuntimePlatformRequest::new(
                    playback_runtime::RuntimePlatformEffect::MidiListOutputsRequest,
                    "test-output".into(),
                    None,
                ),
                DesktopPlatformServiceKind::MidiListOutputs,
            ),
            "service down".into(),
        ));
        assert!(
            matches!(outputs, RuntimeStoreResult::RuntimeFailure { error } if error.message.as_deref() == Some("service down"))
        );

        let inputs = only_result(shape_service_unavailable_result(
            DesktopPlatformServiceRequest::new(
                RuntimePlatformRequest::new(
                    playback_runtime::RuntimePlatformEffect::MidiListInputsRequest,
                    "test-input".into(),
                    None,
                ),
                DesktopPlatformServiceKind::MidiListInputs,
            ),
            "service down".into(),
        ));
        assert!(
            matches!(inputs, RuntimeStoreResult::RuntimeFailure { error } if error.message.as_deref() == Some("service down"))
        );
    }

    #[test]
    fn service_unavailable_shapes_sample_list_error() {
        let result = only_result(shape_service_unavailable_result(
            DesktopPlatformServiceRequest::new(
                RuntimePlatformRequest::new(
                    playback_runtime::RuntimePlatformEffect::SampleListRequest {
                        instrument_slot: 2,
                        sample_slot: 3,
                        dir: "kits".into(),
                    },
                    "test-sample".into(),
                    None,
                ),
                DesktopPlatformServiceKind::SampleList {
                    instrument_slot: 2,
                    sample_slot: 3,
                    dir: "kits".into(),
                },
            ),
            "service down".into(),
        ));

        assert!(
            matches!(result, RuntimeStoreResult::SampleListError { instrument_slot: 2, sample_slot: 3, dir, message } if dir == "kits" && message == "service down")
        );
    }
}
