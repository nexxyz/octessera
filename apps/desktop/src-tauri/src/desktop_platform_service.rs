use crate::midi;
use crate::persistence::atomic_write_json;
use crate::samples;
use playback_runtime::{
    HostMessage, MidiPort, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeOperation, RuntimePlatformRequest, RuntimeStoreResult, RuntimeSystemInfo,
    RuntimeSystemInfoError, SampleEntry,
};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread;

mod host_system_info;

use host_system_info::collect_system_info;

const DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY: usize = 32;

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
    SystemInfo,
    SaveSystem {
        store_dir: PathBuf,
        payload: serde_json::Value,
    },
    #[cfg(test)]
    WorkerGate {
        started: SyncSender<()>,
        release: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    },
}

impl DesktopPlatformServiceRequest {
    pub(crate) fn new(request: RuntimePlatformRequest, kind: DesktopPlatformServiceKind) -> Self {
        Self { request, kind }
    }
}

pub(crate) struct DesktopPlatformService {
    pub(crate) request_tx: SyncSender<DesktopPlatformServiceRequest>,
    pub(crate) result_rx: Receiver<Vec<HostMessage>>,
}

pub(crate) fn spawn_desktop_platform_service() -> DesktopPlatformService {
    let (request_tx, request_rx) = mpsc::sync_channel::<DesktopPlatformServiceRequest>(
        DESKTOP_PLATFORM_REQUEST_QUEUE_CAPACITY,
    );
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

pub(crate) fn admit_platform_service_request(
    request_tx: &SyncSender<DesktopPlatformServiceRequest>,
    request: DesktopPlatformServiceRequest,
) -> Result<(), Vec<HostMessage>> {
    match request_tx.try_send(request) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(request)) => Err(shape_service_unavailable_result(
            request,
            "Desktop platform service queue is full.".into(),
        )),
        Err(TrySendError::Disconnected(request)) => Err(shape_service_unavailable_result(
            request,
            "Desktop platform service unavailable".into(),
        )),
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
        DesktopPlatformServiceKind::SystemInfo => shape_system_info_result(collect_system_info),
        DesktopPlatformServiceKind::SaveSystem { store_dir, payload } => {
            shape_save_system_result(&store_dir, &payload)
        }
        #[cfg(test)]
        DesktopPlatformServiceKind::WorkerGate { started, release } => {
            let _ = started.send(());
            let (lock, condition) = &*release;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = condition.wait(released).unwrap();
            }
            Vec::new()
        }
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
        DesktopPlatformServiceKind::SystemInfo => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SystemInfoError {
                error: RuntimeSystemInfoError::unavailable(message),
            },
        }],
        DesktopPlatformServiceKind::SaveSystem { .. } => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure {
                error: runtime_request.failure_facts(message),
            },
        }],
        #[cfg(test)]
        DesktopPlatformServiceKind::WorkerGate { .. } => Vec::new(),
    };
    identify_service_messages(messages, &runtime_request)
}

fn shape_save_system_result(
    store_dir: &std::path::Path,
    payload: &serde_json::Value,
) -> Vec<HostMessage> {
    let result = match atomic_write_json(&store_dir.join("system.json"), payload) {
        Ok(()) => RuntimeStoreResult::SaveSystemResult { ok: true },
        Err(message) => RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Storage,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::StoreSaveSystem,
                Some(message),
            ),
        },
    };
    vec![HostMessage::RuntimeResult { result }]
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

fn shape_system_info_result(
    collect: impl FnOnce() -> Result<RuntimeSystemInfo, String>,
) -> Vec<HostMessage> {
    let result = match collect() {
        Ok(info) => RuntimeStoreResult::SystemInfoResult {
            info: info.sanitized(),
        },
        Err(message) => RuntimeStoreResult::SystemInfoError {
            error: RuntimeSystemInfoError::unavailable(message),
        },
    };
    vec![HostMessage::RuntimeResult { result }]
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
mod admission_tests;
#[cfg(test)]
mod tests;
