use crate::platform_service::platform_native_persistence::{
    NativeAutosaveWrite, NativeDefaultCompletion, PlatformResult,
};
use crate::user_data_transfer::StoreWriteBarrier;
use playback_runtime::{
    HostMessage, NativeStoreRequest, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeStoreResult,
};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub(super) fn run(
    store_dir: &Path,
    write: NativeAutosaveWrite,
    store_lock: &Mutex<()>,
    store_write_barrier: &StoreWriteBarrier,
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
            let payload = match snapshot.into_local_patch_payload() {
                Ok(payload) => payload,
                Err(message) => {
                    append_failed_results(&mut results, default, backup, &message);
                    return results;
                }
            };
            if let Some(request) = default {
                let result = save_default(store_dir, &payload);
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
                let result = match super::platform_service_store::validate_patch_document(
                    store_dir, &payload,
                )
                .and_then(|()| super::platform_service_store::save_backup(store_dir, &payload))
                {
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

fn save_default(store_dir: &Path, payload: &serde_json::Value) -> Result<(), String> {
    super::platform_service_store::validate_patch_document(store_dir, payload)?;
    super::platform_service_store::save_json(
        &super::platform_service_store::default_patch_path(store_dir),
        payload,
    )
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
