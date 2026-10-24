use playback_runtime::{
    NativeConfigSnapshot, NativeManualSaveRequest, NativeRunner, NativeStoreRequest,
    PlaybackRuntime, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimeStoreResult,
};
use serde_json::Value;
use std::sync::Arc;

mod completion;

pub(crate) use completion::finish_platform_result;

pub(crate) enum PlatformWorkItem {
    Legacy(super::PlatformJob),
    NativeDefault {
        request: NativeStoreRequest,
        snapshot: Box<NativeConfigSnapshot>,
        generation: u64,
    },
    NativePreset {
        write: NativePresetWrite,
        snapshot: Box<NativeConfigSnapshot>,
        generation: u64,
    },
    NativeAutosave {
        write: NativeAutosaveWrite,
    },
}

pub(crate) enum PlatformResult {
    Legacy(playback_runtime::HostMessage),
    NativeDefaultCompletion(NativeDefaultCompletion),
    NativePresetCompletion(NativePresetCompletion),
}

pub(crate) struct NativeDefaultCompletion {
    pub(crate) result: RuntimeStoreResult,
    pub(crate) prepared: Option<Arc<Value>>,
}

#[derive(Clone)]
pub(crate) struct NativePresetWrite {
    pub(crate) request: NativeStoreRequest,
    pub(crate) manual: NativeManualSaveRequest,
}

pub(crate) struct NativeAutosaveWrite {
    pub(crate) default: Option<NativeStoreRequest>,
    pub(crate) backup: Option<NativeStoreRequest>,
    pub(crate) snapshot: Box<NativeConfigSnapshot>,
    pub(crate) generation: u64,
}

pub(crate) struct NativePresetCompletion {
    pub(crate) result: RuntimeStoreResult,
    pub(crate) names: Vec<String>,
    pub(crate) cleanup_error: Option<String>,
}

impl super::PiPlatformService {
    pub(crate) fn enqueue_native_default(
        &self,
        request: NativeStoreRequest,
        snapshot: NativeConfigSnapshot,
    ) -> Result<(), String> {
        if self.store_write_barrier.is_blocked() {
            return Err("restore is awaiting restored-state acknowledgement".into());
        }
        let mut current = self
            .native_default_write
            .lock()
            .map_err(|_| "native default save state is unavailable".to_string())?;
        if current.is_some() {
            return Err("native default save is already pending".into());
        }
        *current = Some(request.clone());
        self.jobs
            .try_send(super::PlatformWorkItem::NativeDefault {
                request,
                snapshot: Box::new(snapshot),
                generation: self.store_write_barrier.current_generation(),
            })
            .map_err(|error| {
                *current = None;
                match error {
                    std::sync::mpsc::TrySendError::Full(_) => {
                        "pi platform service queue is full".to_string()
                    }
                    std::sync::mpsc::TrySendError::Disconnected(_) => {
                        "pi platform service stopped".to_string()
                    }
                }
            })
    }

    pub(crate) fn native_default_write(&self) -> Option<NativeStoreRequest> {
        self.native_default_write
            .lock()
            .expect("native default save state lock is poisoned")
            .clone()
    }

    pub(crate) fn enqueue_native_preset(
        &self,
        write: NativePresetWrite,
        snapshot: NativeConfigSnapshot,
    ) -> Result<(), String> {
        if self.store_write_barrier.is_blocked() {
            return Err("restore is awaiting restored-state acknowledgement".into());
        }
        let mut current = self
            .native_preset_write
            .lock()
            .map_err(|_| "native preset save state is unavailable".to_string())?;
        if current.is_some() {
            return Err("native preset save is already pending".into());
        }
        *current = Some(write.clone());
        self.jobs
            .try_send(super::PlatformWorkItem::NativePreset {
                write,
                snapshot: Box::new(snapshot),
                generation: self.store_write_barrier.current_generation(),
            })
            .map_err(|error| {
                *current = None;
                match error {
                    std::sync::mpsc::TrySendError::Full(_) => {
                        "pi platform service queue is full".to_string()
                    }
                    std::sync::mpsc::TrySendError::Disconnected(_) => {
                        "pi platform service stopped".to_string()
                    }
                }
            })
    }

    pub(crate) fn native_preset_write(&self) -> Option<NativePresetWrite> {
        self.native_preset_write
            .lock()
            .expect("native preset save state lock is poisoned")
            .clone()
    }

    pub(crate) fn clear_native_preset_write(&self, write: &NativePresetWrite) {
        let mut current = self
            .native_preset_write
            .lock()
            .expect("native preset save state lock is poisoned");
        if current.as_ref().is_some_and(|current| {
            current.request == write.request && current.manual == write.manual
        }) {
            *current = None;
        }
    }

    pub(crate) fn clear_native_default_write(&self, request: &NativeStoreRequest) {
        let mut current = self
            .native_default_write
            .lock()
            .expect("native default save state lock is poisoned");
        if current.as_ref() == Some(request) {
            *current = None;
        }
    }
}

pub(crate) fn submit_manual_save(
    service: &super::PiPlatformService,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    manual: NativeManualSaveRequest,
) -> Option<playback_runtime::HostMessage> {
    match manual {
        NativeManualSaveRequest::Default => {
            let snapshot = runner.capture_config_snapshot();
            let revision = snapshot.revision();
            let request =
                playback.next_native_store_request(RuntimeOperation::StoreSaveDefault, revision);
            submit_native_default(service, runner, request, snapshot)
        }
        manual @ NativeManualSaveRequest::Preset { .. } => {
            let snapshot = runner.capture_config_snapshot();
            let request = playback
                .next_native_store_request(RuntimeOperation::StoreSavePreset, snapshot.revision());
            submit_native_preset(service, runner, request, snapshot, manual)
        }
    }
}

pub(crate) fn submit_native_preset(
    service: &super::PiPlatformService,
    runner: &mut NativeRunner,
    request: NativeStoreRequest,
    snapshot: NativeConfigSnapshot,
    manual: NativeManualSaveRequest,
) -> Option<playback_runtime::HostMessage> {
    if !runner.register_native_preset_write(
        request.request_id(),
        request.revision(),
        manual.clone(),
    ) {
        return Some(failure(
            &request,
            "native preset save registration failed".into(),
        ));
    }
    service
        .enqueue_native_preset(
            NativePresetWrite {
                request: request.clone(),
                manual,
            },
            snapshot,
        )
        .err()
        .map(|message| failure(&request, message))
}

pub(crate) fn submit_native_default(
    service: &super::PiPlatformService,
    runner: &mut NativeRunner,
    request: NativeStoreRequest,
    snapshot: NativeConfigSnapshot,
) -> Option<playback_runtime::HostMessage> {
    if !runner.register_native_default_write(request.request_id(), request.revision(), false) {
        return Some(failure(
            &request,
            "native default save registration failed".into(),
        ));
    }
    service
        .enqueue_native_default(request.clone(), snapshot)
        .err()
        .map(|message| failure(&request, message))
}

#[cfg(test)]
mod preset_tests;
#[cfg(test)]
mod tests;

pub(crate) fn failure(
    request: &NativeStoreRequest,
    message: String,
) -> playback_runtime::HostMessage {
    playback_runtime::HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Storage,
                RuntimeErrorCode::OperationFailed,
                request.operation().clone(),
                Some(message),
            )
            .with_identity(Some(request.request_id().into()), Some(request.revision())),
        }
        .with_identity(request.request_id().into(), Some(request.revision())),
    }
}
