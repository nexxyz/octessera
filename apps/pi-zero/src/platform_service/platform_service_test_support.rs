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
}
