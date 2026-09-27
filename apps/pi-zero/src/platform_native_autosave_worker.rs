use crate::platform_service::platform_native_persistence::{
    NativeAutosaveWrite, NativeDefaultCompletion, PlatformResult,
};
use crate::platform_service::platform_service_worker::PlatformWorkerConfig;
use crate::user_data_transfer::StoreWriteBarrier;
use playback_runtime::{
    HostMessage, NativeStoreRequest, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeStoreResult,
};
use serde_json::Value;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub(super) fn run(
    store_dir: &Path,
    write: NativeAutosaveWrite,
    store_lock: &Mutex<()>,
    store_write_barrier: &StoreWriteBarrier,
    worker_config: &PlatformWorkerConfig,
) -> Vec<PlatformResult> {
    let NativeAutosaveWrite {
        default,
        backup,
        snapshot,
        generation,
    } = write;
    let mut results = Vec::with_capacity(2);
    match store_lock.lock() {
        Ok(_guard)
            if generation == store_write_barrier.current_generation()
                && !store_write_barrier.is_blocked() =>
        {
            let payload = snapshot.into_payload();
            if let Some(request) = default {
                let result = save_default(store_dir, &payload, &request, worker_config);
                let prepared = result.as_ref().ok().map(|()| Arc::new(payload.clone()));
                let result = match result {
                    Ok(()) => RuntimeStoreResult::SaveDefaultResult {
                        ok: true,
                        is_auto: Some(true),
                    },
                    Err(message) => failure(&request, message),
                }
                .with_identity(request.request_id().into(), Some(request.revision()));
                results.push(PlatformResult::NativeDefaultCompletion(
                    NativeDefaultCompletion { result, prepared },
                ));
            }
            if let Some(request) = backup {
                let result =
                    match super::platform_service_store::save_backup(store_dir, &payload) {
                        Ok(()) => RuntimeStoreResult::SaveBackupResult { ok: true },
                        Err(message) => failure(&request, message),
                    }
                    .with_identity(request.request_id().into(), Some(request.revision()));
                results.push(PlatformResult::Legacy(HostMessage::RuntimeResult {
                    result,
                }));
            }
        }
        Ok(_) => append_failed_results(
            &mut results,
            default,
            backup,
            "store write cancelled because restore was confirmed",
        ),
        Err(_) => append_failed_results(&mut results, default, backup, "pi store is unavailable"),
    }
    results
}

fn save_default(
    store_dir: &Path,
    payload: &Value,
    request: &NativeStoreRequest,
    worker_config: &PlatformWorkerConfig,
) -> Result<(), String> {
    let _ = request;
    crate::usb_config_validation::validate_pi_audio_outputs_payload(payload)?;
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    if let Some(result) = crate::rpi_device_apply::save_default_if_role_changed(
        store_dir,
        payload,
        None,
        &worker_config.storage_state,
        worker_config.role_applier.as_ref(),
    ) {
        return match result {
            RuntimeStoreResult::SaveDefaultResult { ok: true, .. } => Ok(()),
            RuntimeStoreResult::StoreError { message } => Err(message),
            _ => Err("default save did not complete".into()),
        };
    }
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let _ = (request, worker_config);
    super::platform_service_store::save_json(&store_dir.join("default.json"), payload)
}

fn append_failed_results(
    results: &mut Vec<PlatformResult>,
    default: Option<NativeStoreRequest>,
    backup: Option<NativeStoreRequest>,
    message: &str,
) {
    if let Some(request) = default {
        results.push(PlatformResult::NativeDefaultCompletion(
            NativeDefaultCompletion {
                result: failure(&request, message.into())
                    .with_identity(request.request_id().into(), Some(request.revision())),
                prepared: None,
            },
        ));
    }
    if let Some(request) = backup {
        let result = failure(&request, message.into())
            .with_identity(request.request_id().into(), Some(request.revision()));
        results.push(PlatformResult::Legacy(HostMessage::RuntimeResult {
            result,
        }));
    }
}

fn failure(request: &NativeStoreRequest, message: String) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Storage,
            RuntimeErrorCode::OperationFailed,
            request.operation().clone(),
            Some(message),
        ),
    }
}
