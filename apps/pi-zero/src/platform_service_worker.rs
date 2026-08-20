use super::*;
use std::thread;

pub(super) fn spawn(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformJob>,
    results: Arc<PlatformResultLane>,
    store_lock: Arc<Mutex<()>>,
    store_write_barrier: StoreWriteBarrier,
    update_executor: Arc<dyn device_update::UpdateExecutor>,
) {
    thread::spawn(move || {
        run(
            store_dir,
            samples_dir,
            jobs,
            results,
            store_lock,
            store_write_barrier,
            update_executor,
        )
    });
}

fn run(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformJob>,
    results: Arc<PlatformResultLane>,
    store_lock: Arc<Mutex<()>>,
    store_write_barrier: StoreWriteBarrier,
    update_executor: Arc<dyn device_update::UpdateExecutor>,
) {
    while let Ok(job) = jobs.recv() {
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
                        platform_service_executor::handle_job(
                            &store_dir,
                            &samples_dir,
                            job,
                            update_executor.as_ref(),
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
                update_executor.as_ref(),
            )
        };
        if results
            .send_platform(HostMessage::RuntimeResult { result })
            .is_err()
        {
            break;
        }
    }
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
