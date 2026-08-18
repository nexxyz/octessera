use super::*;
use std::thread;

pub(super) fn spawn(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformJob>,
    results: Arc<PlatformResultLane>,
    update_executor: Arc<dyn device_update::UpdateExecutor>,
) {
    thread::spawn(move || run(store_dir, samples_dir, jobs, results, update_executor));
}

fn run(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformJob>,
    results: Arc<PlatformResultLane>,
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
            let result = crate::orange_device_apply::prepare(&store_dir, payload);
            let _ = completed.send(result);
            continue;
        }
        let result = handle_job(&store_dir, &samples_dir, job, update_executor.as_ref());
        if results
            .send_platform(HostMessage::RuntimeResult { result })
            .is_err()
        {
            break;
        }
    }
}
