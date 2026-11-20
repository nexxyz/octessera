use super::*;
use playback_runtime::NativeConfigSnapshot;
use std::path::Path;
use std::thread;

pub(super) struct PlatformWorkerConfig {
    pub(super) update_executor: Arc<dyn device_update::UpdateExecutor>,
    pub(super) device_apply: crate::device_apply::DeviceApply,
}

pub(super) struct PlatformWorkerContext {
    pub(super) store_dir: PathBuf,
    pub(super) samples_dir: PathBuf,
    pub(super) jobs: Receiver<PlatformWorkItem>,
    pub(super) results: Arc<PlatformResultLane>,
    pub(super) store_lock: Arc<Mutex<()>>,
    pub(super) store_write_barrier: StoreWriteBarrier,
    pub(super) legacy_patch_writes: Arc<Mutex<super::LegacyPatchWrites>>,
    pub(super) update_executor: Arc<dyn device_update::UpdateExecutor>,
    pub(super) device_apply: crate::device_apply::DeviceApply,
}

pub(super) fn spawn(context: PlatformWorkerContext) {
    thread::spawn(move || {
        lower_priority();
        run(context)
    });
}

/// Store jobs share the app's CPU with the runtime loop. At its priority, the rolling backup's
/// validate, serialize and write held dispatches off for up to 2.5 ms.
fn lower_priority() {
    #[cfg(target_os = "linux")]
    // SAFETY: setpriority takes integers only; who 0 names the calling thread.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 10);
    }
}

fn run(context: PlatformWorkerContext) {
    let PlatformWorkerContext {
        store_dir,
        samples_dir,
        jobs,
        results,
        store_lock,
        store_write_barrier,
        legacy_patch_writes,
        update_executor,
        device_apply,
    } = context;
    while let Ok(item) = jobs.recv() {
        let job = match item {
            PlatformWorkItem::Legacy(job) => job,
            PlatformWorkItem::NativeDefault {
                request,
                snapshot,
                generation,
            } => {
                let completion = native_default_completion(
                    &store_dir,
                    request,
                    snapshot,
                    generation,
                    &store_lock,
                    &store_write_barrier,
                );
                if results
                    .send_platform(PlatformResult::NativeDefaultCompletion(
                        NativeDefaultCompletion {
                            result: completion.0,
                            prepared: completion.1,
                        },
                    ))
                    .is_err()
                {
                    break;
                }
                continue;
            }
            PlatformWorkItem::NativePreset {
                write,
                snapshot,
                generation,
            } => {
                let result = native_preset_worker_result(
                    &store_dir,
                    write,
                    snapshot,
                    generation,
                    &store_lock,
                    &store_write_barrier,
                );
                if results.send_platform(result).is_err() {
                    break;
                }
                continue;
            }
            PlatformWorkItem::NativeAutosave { write } => {
                let native_results = super::platform_native_autosave_worker::run(
                    &store_dir,
                    write,
                    &store_lock,
                    &store_write_barrier,
                );
                for result in native_results {
                    if results.send_platform(result).is_err() {
                        return;
                    }
                }
                continue;
            }
        };
        if let PlatformJobKind::QueueBarrier { completed } = &job.kind {
            let _ = completed.send(());
            continue;
        }
        #[cfg(test)]
        if let PlatformJobKind::TestGate { entered, release } = &job.kind {
            let _ = entered.send(());
            let _ = release.recv();
            continue;
        }
        if matches!(
            job.kind,
            PlatformJobKind::UpdateCheck | PlatformJobKind::UpdateApply | PlatformJobKind::Rollback
        ) {
            spawn_update_job(&store_dir, &samples_dir, job, &results, &update_executor);
            continue;
        }
        let result = if let PlatformJobKind::ApplyDeviceConfig {
            payload,
            usb_change,
        } = &job.kind
        {
            device_apply
                .apply(*usb_change, || match store_lock.lock() {
                    Ok(_guard) => match superseded_store_write(&job, &store_write_barrier) {
                        Some(_) => {
                            Err("store write cancelled because restore was confirmed".into())
                        }
                        None => super::platform_service_document_store::save_system_document(
                            &store_dir, payload,
                        ),
                    },
                    Err(_) => Err("pi store is unavailable".into()),
                })
                .with_identity(job.request.request_id.clone(), job.request.revision)
        } else if job_requires_store_lock(&job.kind) {
            match store_lock.lock() {
                Ok(_guard) => {
                    let write_kind = legacy_patch_write_kind(&job.kind);
                    let result =
                        if let Some(result) = superseded_store_write(&job, &store_write_barrier) {
                            result
                        } else {
                            platform_service_executor::handle_job(
                                &store_dir,
                                &samples_dir,
                                job,
                                update_executor.as_ref(),
                            )
                        };
                    decrement_legacy_patch_write(&legacy_patch_writes, write_kind);
                    result
                }
                Err(_) => RuntimeStoreResult::RuntimeFailure {
                    error: job.request.failure_facts("pi store is unavailable".into()),
                }
                .with_identity(job.request.request_id.clone(), job.request.revision),
            }
        } else {
            platform_service_executor::handle_job(
                &store_dir,
                &samples_dir,
                job,
                update_executor.as_ref(),
            )
        };
        if results
            .send_platform(PlatformResult::Legacy(HostMessage::RuntimeResult {
                result,
            }))
            .is_err()
        {
            break;
        }
    }
}

/// Update helpers download releases, so they run beside the store worker and saves never wait
/// for the network; the helper's own lock keeps concurrent update actions apart.
fn spawn_update_job(
    store_dir: &Path,
    samples_dir: &Path,
    job: PlatformJob,
    results: &Arc<PlatformResultLane>,
    update_executor: &Arc<dyn device_update::UpdateExecutor>,
) {
    let store_dir = store_dir.to_path_buf();
    let samples_dir = samples_dir.to_path_buf();
    let results = results.clone();
    let update_executor = update_executor.clone();
    let request = job.request.clone();
    let spawned = thread::Builder::new()
        .name("octessera-device-update".into())
        .spawn({
            let results = results.clone();
            move || {
                lower_priority();
                let result = platform_service_executor::handle_job(
                    &store_dir,
                    &samples_dir,
                    job,
                    update_executor.as_ref(),
                );
                let _ = results.send_platform(PlatformResult::Legacy(HostMessage::RuntimeResult {
                    result,
                }));
            }
        });
    if let Err(error) = spawned {
        let result = RuntimeStoreResult::RuntimeFailure {
            error: request.failure_facts(format!("device update could not start: {error}")),
        }
        .with_identity(request.request_id.clone(), request.revision);
        let _ = results.send_platform(PlatformResult::Legacy(HostMessage::RuntimeResult {
            result,
        }));
    }
}

fn legacy_patch_write_kind(kind: &PlatformJobKind) -> Option<bool> {
    match kind {
        PlatformJobKind::SaveDefault { .. } => Some(false),
        PlatformJobKind::SavePreset { .. } => Some(true),
        _ => None,
    }
}

fn decrement_legacy_patch_write(
    pending: &Arc<Mutex<super::LegacyPatchWrites>>,
    kind: Option<bool>,
) {
    if let Some(named) = kind {
        if let Ok(mut pending) = pending.lock() {
            pending.decrement(named);
        }
    }
}

fn native_default_completion(
    store_dir: &Path,
    request: NativeStoreRequest,
    snapshot: Box<NativeConfigSnapshot>,
    generation: u64,
    store_lock: &Mutex<()>,
    store_write_barrier: &StoreWriteBarrier,
) -> (RuntimeStoreResult, Option<Arc<serde_json::Value>>) {
    let result = match store_lock.lock() {
        Ok(_guard)
            if generation == store_write_barrier.current_generation()
                && !store_write_barrier.is_blocked() =>
        {
            let saved = snapshot.into_local_patch_payload().and_then(|payload| {
                super::platform_service_store::validate_patch_document(store_dir, &payload)?;
                super::platform_service_store::save_json(
                    &super::platform_service_store::default_patch_path(store_dir),
                    &payload,
                )?;
                Ok(payload)
            });
            match saved {
                Ok(payload) => (
                    RuntimeStoreResult::SaveDefaultResult {
                        ok: true,
                        is_auto: None,
                    },
                    Some(Arc::new(playback_runtime::saved_patch_header(&payload))),
                ),
                Err(message) => (native_failure(&request, message), None),
            }
        }
        Ok(_) => (
            native_failure(
                &request,
                "store write cancelled because restore was confirmed".into(),
            ),
            None,
        ),
        Err(_) => (
            native_failure(&request, "pi store is unavailable".into()),
            None,
        ),
    };
    (
        result
            .0
            .with_identity(request.request_id().to_string(), Some(request.revision())),
        result.1,
    )
}

fn native_failure(request: &NativeStoreRequest, message: String) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: playback_runtime::RuntimeErrorFacts::new(
            playback_runtime::RuntimeErrorDomain::Storage,
            playback_runtime::RuntimeErrorCode::OperationFailed,
            request.operation().clone(),
            Some(message),
        )
        .with_preset_name(request.preset_name().map(str::to_owned)),
    }
}

fn native_preset_worker_result(
    store_dir: &Path,
    write: NativePresetWrite,
    snapshot: Box<NativeConfigSnapshot>,
    generation: u64,
    store_lock: &Mutex<()>,
    store_write_barrier: &StoreWriteBarrier,
) -> PlatformResult {
    let completion = match store_lock.lock() {
        Ok(_guard)
            if generation == store_write_barrier.current_generation()
                && !store_write_barrier.is_blocked() =>
        {
            save_native_preset(store_dir, &write, *snapshot)
        }
        Ok(_) => Err("store write cancelled because restore was confirmed".into()),
        Err(_) => Err("pi store is unavailable".into()),
    };
    match completion {
        Ok(completion) => PlatformResult::NativePresetCompletion(completion),
        Err(message) => PlatformResult::Legacy(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure {
                error: playback_runtime::RuntimeErrorFacts::new(
                    playback_runtime::RuntimeErrorDomain::Storage,
                    playback_runtime::RuntimeErrorCode::OperationFailed,
                    RuntimeOperation::StoreSavePreset,
                    Some(message),
                )
                .with_preset_name(write.request.preset_name().map(str::to_owned))
                .with_identity(
                    Some(write.request.request_id().into()),
                    Some(write.request.revision()),
                ),
            }
            .with_identity(
                write.request.request_id().into(),
                Some(write.request.revision()),
            ),
        }),
    }
}

fn save_native_preset(
    store_dir: &Path,
    write: &NativePresetWrite,
    snapshot: NativeConfigSnapshot,
) -> Result<NativePresetCompletion, String> {
    let NativeManualSaveRequest::Preset {
        name,
        mode,
        rename_from,
    } = &write.manual
    else {
        return Err("native preset worker received a default save".into());
    };
    let target = super::platform_service_store::preset_patch_path(store_dir, name.as_str())
        .map_err(|error| format!("Save {name} failed: {error}"))?;
    let existed = target.is_file();
    if existed && mode.is_none() {
        return Err(format!("Save {name} failed: preset already exists"));
    }
    if let Some(source) = rename_from
        .as_deref()
        .filter(|source| *source != name.as_str())
    {
        super::platform_service_store::preset_patch_path(store_dir, source)
            .map_err(|error| format!("Rename {source} failed: {error}"))?;
    }
    let payload = snapshot
        .into_portable_patch_payload()
        .map_err(|error| format!("Save {name} failed: {error}"))?;
    super::platform_service_store::validate_named_preset_document(store_dir, &payload)
        .map_err(|error| format!("Save {name} failed: {error}"))?;
    super::platform_service_store::save_json(&target, &payload)
        .map_err(|error| format!("Save {name} failed: {error}"))?;
    let cleanup_error = rename_from
        .as_deref()
        .filter(|source| *source != name.as_str())
        .and_then(|source| {
            super::platform_service_store::delete_preset_payload_result(store_dir, source).err()
        });
    let names = super::platform_service_store::list_presets(store_dir)
        .map_err(|error| format!("Preset catalog refresh failed: {error}"))?;
    Ok(NativePresetCompletion {
        result: RuntimeStoreResult::SavePresetResult {
            name: name.clone(),
            outcome: if existed { "overwritten" } else { "created" }.into(),
        }
        .with_identity(
            write.request.request_id().into(),
            Some(write.request.revision()),
        ),
        names,
        cleanup_error,
    })
}

fn job_requires_store_lock(kind: &PlatformJobKind) -> bool {
    matches!(
        kind,
        PlatformJobKind::ListPresets
            | PlatformJobKind::SavePreset { .. }
            | PlatformJobKind::DeletePreset { .. }
            | PlatformJobKind::SaveDefault { .. }
            | PlatformJobKind::SaveSystem { .. }
            | PlatformJobKind::SaveBackup { .. }
            | PlatformJobKind::ListSamples { .. }
    )
}

fn superseded_store_write(
    job: &PlatformJob,
    store_write_barrier: &StoreWriteBarrier,
) -> Option<RuntimeStoreResult> {
    let generation = job.store_write_generation?;
    if generation == store_write_barrier.current_generation() {
        return None;
    }
    Some(
        RuntimeStoreResult::RuntimeFailure {
            error: job
                .request
                .failure_facts("store write cancelled because restore was confirmed".into()),
        }
        .with_identity(job.request.request_id.clone(), job.request.revision),
    )
}
