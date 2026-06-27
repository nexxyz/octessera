use super::RuntimeWorker;
use std::sync::mpsc;

const MAX_PLATFORM_SERVICE_RESULTS_PER_LOOP: usize = 4;

impl RuntimeWorker {
    pub(super) fn poll_platform_service_results(&mut self) {
        for _ in 0..MAX_PLATFORM_SERVICE_RESULTS_PER_LOOP {
            match self.platform_service_result_rx.try_recv() {
                Ok(follow_ups) => match self.dispatch_follow_ups(follow_ups) {
                    Ok(returned) => {
                        if let Err(error) = self.emit_runner_messages(returned) {
                            self.record_nonfatal_platform_service_error(error);
                        }
                    }
                    Err(error) => self.record_nonfatal_platform_service_error(error),
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
    }

    fn record_nonfatal_platform_service_error(&mut self, error: String) {
        eprintln!("desktop platform service result handling failed: {error}");
        if let Ok(mut guard) = self.audio_error.lock() {
            *guard = Some(error);
        }
    }
}
