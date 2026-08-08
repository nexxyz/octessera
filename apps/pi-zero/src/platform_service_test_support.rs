use super::*;
use playback_runtime::RuntimePlatformEffect;

impl PiPlatformService {
    pub(crate) fn enqueue_test_barrier(&self) -> Result<Receiver<()>, String> {
        let (completed_tx, completed_rx) = mpsc::sync_channel(0);
        let request = RuntimePlatformRequest::new(
            RuntimePlatformEffect::UpdateCheck,
            "test-service-barrier".into(),
            None,
        );
        self.enqueue(PlatformJob::new(
            request,
            PlatformJobKind::TestBarrier {
                completed: completed_tx,
            },
        ))?;
        Ok(completed_rx)
    }

    pub(crate) fn enqueue_test_gate(&self) -> Result<(Receiver<()>, SyncSender<()>), String> {
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let request = RuntimePlatformRequest::new(
            RuntimePlatformEffect::UpdateCheck,
            "test-service-gate".into(),
            None,
        );
        self.enqueue(PlatformJob::new(
            request,
            PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))?;
        Ok((entered_rx, release_tx))
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn disconnect_results_for_test(&mut self) {
        let (_, replacement) = mpsc::sync_channel(1);
        let results = std::mem::replace(&mut self.results, replacement);
        drop(results);
    }
}
