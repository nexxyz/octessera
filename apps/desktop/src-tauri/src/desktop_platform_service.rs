use crate::midi;
use crate::samples;
use playback_runtime::{
    HostMessage, MidiPort, RuntimePlatformRequest, RuntimeStoreResult, RuntimeSystemInfo,
    RuntimeSystemInfoError, SampleEntry,
};
use std::net::UdpSocket;
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
    SystemInfo,
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
        DesktopPlatformServiceKind::SystemInfo => shape_system_info_result(collect_system_info),
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

fn collect_system_info() -> Result<RuntimeSystemInfo, String> {
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unavailable".into());
    let primary_ip = primary_ip();
    let primary_mac = primary_mac();
    Ok(RuntimeSystemInfo {
        os: std::env::consts::OS.into(),
        os_version: os_version(),
        octessera_version: env!("CARGO_PKG_VERSION").into(),
        primary_ip,
        primary_mac,
        hostname,
        board_profile: "desktop-simulator".into(),
    })
}

fn os_version() -> String {
    #[cfg(target_os = "windows")]
    let command = ("cmd", vec!["/C", "ver"]);
    #[cfg(not(target_os = "windows"))]
    let command = ("uname", vec!["-sr"]);
    std::process::Command::new(command.0)
        .args(command.1)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "unavailable".into())
}

fn primary_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_loopback()).then(|| address.to_string())
}

fn primary_mac() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("getmac")
            .args(["/fo", "csv", "/nh"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout)
            .split(|character: char| {
                !character.is_ascii_hexdigit() && character != '-' && character != ':'
            })
            .find(|value| is_mac(value))
            .map(ToOwned::to_owned)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut interfaces = std::fs::read_dir("/sys/class/net")
            .ok()?
            .flatten()
            .collect::<Vec<_>>();
        interfaces.sort_by_key(|entry| entry.file_name());
        for interface in interfaces {
            if interface.file_name() == "lo" {
                continue;
            }
            let Ok(address) = std::fs::read_to_string(interface.path().join("address")) else {
                continue;
            };
            let address = address.trim();
            if is_mac(address) {
                return Some(address.to_string());
            }
        }
        None
    }
}

fn is_mac(value: &str) -> bool {
    let octets = value.split([':', '-']).collect::<Vec<_>>();
    octets.len() == 6
        && octets
            .iter()
            .all(|octet| octet.len() == 2 && octet.chars().all(|c| c.is_ascii_hexdigit()))
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

    #[test]
    fn system_info_service_sanitizes_successful_adapter_data() {
        let messages = shape_system_info_result(|| {
            Ok(RuntimeSystemInfo {
                os: "Linux\nnoise".into(),
                os_version: "6.6".into(),
                octessera_version: "0.7.0".into(),
                primary_ip: None,
                primary_mac: None,
                hostname: "octessera".into(),
                board_profile: "desktop".into(),
            })
        });
        let result = only_result(messages);
        assert!(matches!(
            result,
            RuntimeStoreResult::SystemInfoResult { info }
                if info.os == "Linuxnoise" && info.board_profile == "desktop"
        ));
    }

    #[test]
    fn system_info_service_shapes_typed_unavailable_error() {
        let result = only_result(shape_system_info_result(|| Err("service down".into())));
        assert!(matches!(
            result,
            RuntimeStoreResult::SystemInfoError { error }
                if error.code == playback_runtime::RuntimeErrorCode::Unavailable
                    && error.message == "service down"
        ));
    }

    #[test]
    fn system_info_service_result_keeps_request_identity() {
        let messages = shape_service_result(DesktopPlatformServiceRequest::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "system-info-test".into(),
                Some(4),
            ),
            DesktopPlatformServiceKind::SystemInfo,
        ));
        assert!(matches!(
            messages.as_slice(),
            [HostMessage::RuntimeResult {
                result: RuntimeStoreResult::Identified { request_id, revision, result }
            }] if request_id == "system-info-test"
                && *revision == Some(4)
                && matches!(result.as_ref(), RuntimeStoreResult::SystemInfoResult { .. })
        ));
    }
}
