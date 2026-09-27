use super::*;
use playback_runtime::NativeConfigSnapshot;
use std::path::Path;
use std::thread;

pub(super) struct PlatformWorkerConfig {
    pub(super) update_executor: Arc<dyn device_update::UpdateExecutor>,
    pub(super) role_applier: super::UsbRoleApplier,
    pub(super) storage_state: PathBuf,
}

pub(super) fn spawn(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformWorkItem>,
    results: Arc<PlatformResultLane>,
    store_lock: Arc<Mutex<()>>,
    store_write_barrier: StoreWriteBarrier,
    worker_config: PlatformWorkerConfig,
) {
    thread::spawn(move || {
        run(
            store_dir,
            samples_dir,
            jobs,
            results,
            store_lock,
            store_write_barrier,
            worker_config,
        )
    });
}

fn run(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformWorkItem>,
    results: Arc<PlatformResultLane>,
    store_lock: Arc<Mutex<()>>,
    store_write_barrier: StoreWriteBarrier,
    worker_config: PlatformWorkerConfig,
) {
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
                    &worker_config,
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
                    &worker_config,
                );
                for result in native_results {
                    if results.send_platform(result).is_err() {
                        return;
                    }
                }
                continue;
            }
        };
        #[cfg(test)]
        if let PlatformJobKind::TestBarrier { completed } = &job.kind {
            let _ = completed.send(());
            continue;
        }
        #[cfg(test)]
        if let PlatformJobKind::TestGate { entered, release } = &job.kind {
            let _ = entered.send(());
            let _ = release.recv();
            continue;
        }
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        if let PlatformJobKind::PrepareOrangeDeviceApply { payload, completed } = &job.kind {
            let result = match store_lock.lock() {
                Ok(_guard) => {
                    if superseded_store_write(&job, &store_write_barrier).is_some() {
                        Err("store write cancelled because restore was confirmed".into())
                    } else {
                        crate::orange_device_apply::prepare(&store_dir, payload, store_lock.clone())
                    }
                }
                Err(_) => Err("pi store is unavailable".to_string()),
            };
            let _ = completed.send(result);
            continue;
        }
        let result = if job_requires_store_lock(&job.kind) {
            match store_lock.lock() {
                Ok(_guard) => {
                    if let Some(result) = superseded_store_write(&job, &store_write_barrier) {
                        result
                    } else {
                        handle_job(
                            &store_dir,
                            &samples_dir,
                            job,
                            worker_config.update_executor.as_ref(),
                            &worker_config.role_applier,
                            &worker_config.storage_state,
                        )
                    }
                }
                Err(_) => RuntimeStoreResult::RuntimeFailure {
                    error: job.request.failure_facts("pi store is unavailable".into()),
                },
            }
        } else {
            platform_service_executor::handle_job(
                &store_dir,
                &samples_dir,
                job,
                worker_config.update_executor.as_ref(),
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

fn native_default_completion(
    store_dir: &Path,
    request: NativeStoreRequest,
    snapshot: Box<NativeConfigSnapshot>,
    generation: u64,
    store_lock: &Mutex<()>,
    store_write_barrier: &StoreWriteBarrier,
    worker_config: &PlatformWorkerConfig,
) -> (RuntimeStoreResult, Option<Arc<serde_json::Value>>) {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let _ = worker_config;
    let result = match store_lock.lock() {
        Ok(_guard)
            if generation == store_write_barrier.current_generation()
                && !store_write_barrier.is_blocked() =>
        {
            let payload = snapshot.into_payload();
            let saved = crate::usb_config_validation::validate_pi_audio_outputs_payload(&payload)
                .and_then(|()| {
                    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
                    if let Some(result) = crate::rpi_device_apply::save_default_if_role_changed(
                        store_dir,
                        &payload,
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
                    super::platform_service_store::save_json(
                        &store_dir.join("default.json"),
                        &payload,
                    )
                });
            match saved {
                Ok(()) => (
                    RuntimeStoreResult::SaveDefaultResult {
                        ok: true,
                        is_auto: None,
                    },
                    Some(Arc::new(payload)),
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
        ),
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
    if mode.as_deref().is_some_and(|mode| mode != "overwrite") {
        return Err(format!("Save {name} failed: unsupported overwrite mode"));
    }
    let target = super::platform_service_store::preset_patch_path(store_dir, name.as_str())
        .map_err(|error| format!("Save {name} failed: {error}"))?;
    let existed = target.is_file();
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

fn handle_job(
    store_dir: &Path,
    samples_dir: &Path,
    job: PlatformJob,
    update_executor: &dyn device_update::UpdateExecutor,
    role_applier: &super::UsbRoleApplier,
    storage_state: &Path,
) -> RuntimeStoreResult {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let _ = (role_applier, storage_state);
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    if let PlatformJobKind::SaveDefault { payload, is_auto } = &job.kind {
        if let Some(result) = crate::rpi_device_apply::save_default_if_role_changed(
            store_dir,
            payload,
            *is_auto,
            storage_state,
            role_applier.as_ref(),
        ) {
            let result = match result {
                RuntimeStoreResult::StoreError { message } => RuntimeStoreResult::RuntimeFailure {
                    error: job.request.failure_facts(message),
                },
                result => result,
            };
            return result.with_identity(job.request.request_id.clone(), job.request.revision);
        }
    }
    platform_service_executor::handle_job(store_dir, samples_dir, job, update_executor)
}

fn job_requires_store_lock(kind: &PlatformJobKind) -> bool {
    match kind {
        PlatformJobKind::ListPresets
        | PlatformJobKind::LoadPreset { .. }
        | PlatformJobKind::SavePreset { .. }
        | PlatformJobKind::DeletePreset { .. }
        | PlatformJobKind::SaveDefault { .. }
        | PlatformJobKind::SaveBackup { .. }
        | PlatformJobKind::ListSamples { .. } => true,
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        PlatformJobKind::PrepareOrangeDeviceApply { .. } => true,
        _ => false,
    }
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
