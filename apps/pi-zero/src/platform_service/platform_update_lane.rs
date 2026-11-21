use super::*;
use std::path::Path;
use std::thread;

/// Update helpers download releases, so they run on one update worker beside the store worker
/// and saves never wait for the network. One update runs at a time; another request meanwhile is
/// refused rather than queued behind a download. A worker that panicked is replaced.
#[derive(Default)]
pub(super) struct UpdateLane {
    worker: Option<(SyncSender<PlatformJob>, thread::JoinHandle<()>)>,
    busy: Arc<AtomicBool>,
}

impl UpdateLane {
    pub(super) fn dispatch(
        &mut self,
        store_dir: &Path,
        samples_dir: &Path,
        job: PlatformJob,
        results: &Arc<PlatformResultLane>,
        update_executor: &Arc<dyn device_update::UpdateExecutor>,
    ) {
        if self
            .worker
            .as_ref()
            .is_some_and(|(_, handle)| handle.is_finished())
        {
            self.worker = None;
            self.busy.store(false, Ordering::Release);
        }
        if self.busy.swap(true, Ordering::AcqRel) {
            let result = RuntimeStoreResult::DeviceUpdateStatus {
                ok: false,
                message: "Updater is busy. Try again soon.".into(),
            };
            send_update_result(results, &job.request, result);
            return;
        }
        let (jobs, _) = match &self.worker {
            Some(worker) => worker,
            None => match self.spawn(store_dir, samples_dir, results, update_executor) {
                Ok(worker) => self.worker.insert(worker),
                Err(error) => {
                    self.busy.store(false, Ordering::Release);
                    let result = RuntimeStoreResult::RuntimeFailure {
                        error: job
                            .request
                            .failure_facts(format!("device update could not start: {error}")),
                    };
                    send_update_result(results, &job.request, result);
                    return;
                }
            },
        };
        if let Err(mpsc::SendError(job)) = jobs.send(job) {
            self.worker = None;
            self.busy.store(false, Ordering::Release);
            let result = RuntimeStoreResult::RuntimeFailure {
                error: job
                    .request
                    .failure_facts("device update worker stopped".into()),
            };
            send_update_result(results, &job.request, result);
        }
    }

    fn spawn(
        &self,
        store_dir: &Path,
        samples_dir: &Path,
        results: &Arc<PlatformResultLane>,
        update_executor: &Arc<dyn device_update::UpdateExecutor>,
    ) -> std::io::Result<(SyncSender<PlatformJob>, thread::JoinHandle<()>)> {
        let (jobs_tx, jobs) = mpsc::sync_channel::<PlatformJob>(1);
        let store_dir = store_dir.to_path_buf();
        let samples_dir = samples_dir.to_path_buf();
        let results = results.clone();
        let update_executor = update_executor.clone();
        let busy = self.busy.clone();
        let handle = thread::Builder::new()
            .name("octessera-device-update".into())
            .spawn(move || {
                super::platform_service_worker::lower_priority();
                while let Ok(job) = jobs.recv() {
                    let result = platform_service_executor::handle_job(
                        &store_dir,
                        &samples_dir,
                        job,
                        update_executor.as_ref(),
                    );
                    busy.store(false, Ordering::Release);
                    let _ =
                        results.send_platform(PlatformResult::Legacy(HostMessage::RuntimeResult {
                            result,
                        }));
                }
            })?;
        Ok((jobs_tx, handle))
    }
}

fn send_update_result(
    results: &PlatformResultLane,
    request: &RuntimePlatformRequest,
    result: RuntimeStoreResult,
) {
    let result = result.with_identity(request.request_id.clone(), request.revision);
    let _ = results.send_platform(PlatformResult::Legacy(HostMessage::RuntimeResult {
        result,
    }));
}
