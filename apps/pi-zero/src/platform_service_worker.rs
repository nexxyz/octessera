use super::*;
use std::thread;

pub(super) fn spawn(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformJob>,
    results: SyncSender<HostMessage>,
    update_executor: Arc<dyn device_update::UpdateExecutor>,
) {
    thread::spawn(move || run(store_dir, samples_dir, jobs, results, update_executor));
}

fn run(
    store_dir: PathBuf,
    samples_dir: PathBuf,
    jobs: Receiver<PlatformJob>,
    results: SyncSender<HostMessage>,
    update_executor: Arc<dyn device_update::UpdateExecutor>,
) {
    while let Ok(job) = jobs.recv() {
        #[cfg(test)]
        if let PlatformJobKind::TestBarrier { completed } = &job.kind {
            let _ = completed.send(());
            continue;
        }
        let result = handle_job(&store_dir, &samples_dir, job, update_executor.as_ref());
        if results.send(HostMessage::RuntimeResult { result }).is_err() {
            break;
        }
    }
}
