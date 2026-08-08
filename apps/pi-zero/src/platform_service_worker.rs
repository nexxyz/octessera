use super::*;
use std::thread;

pub(super) fn spawn(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformJob>,
    results: SyncSender<HostMessage>,
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))] update_executor: Arc<
        dyn device_update::UpdateExecutor,
    >,
) {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    thread::spawn(move || run(store_dir, samples_dir, jobs, results, update_executor));
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    thread::spawn(move || run(store_dir, samples_dir, jobs, results));
}

fn run(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformJob>,
    results: SyncSender<HostMessage>,
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))] update_executor: Arc<
        dyn device_update::UpdateExecutor,
    >,
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
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let result = handle_job(&store_dir, &samples_dir, job, update_executor.as_ref());
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let result = handle_job(&store_dir, &samples_dir, job);
        if results.send(HostMessage::RuntimeResult { result }).is_err() {
            break;
        }
    }
}
