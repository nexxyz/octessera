use super::*;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::time::Duration;

impl PiPlatformService {
    pub fn prepare_orange_device_apply(
        &self,
        payload: &serde_json::Value,
    ) -> Result<OrangeDeviceApplyTransaction, String> {
        let (completed_tx, completed_rx) = mpsc::sync_channel(0);
        let request = RuntimePlatformRequest::new(
            playback_runtime::RuntimePlatformEffect::ApplyDeviceConfigReboot {
                payload: payload.clone(),
            },
            "orange-device-apply".into(),
            None,
        );
        self.enqueue(PlatformJob::new(
            request,
            PlatformJobKind::PrepareOrangeDeviceApply {
                payload: payload.clone(),
                completed: completed_tx,
            },
        ))?;
        loop {
            match completed_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    self.preserve_worker_results()?;
                    thread::yield_now();
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Orange platform worker stopped during device apply".into())
                }
            }
        }
    }

    fn preserve_worker_results(&self) -> Result<(), String> {
        let mut preserved = self
            .preserved_results
            .lock()
            .map_err(|_| "Orange preserved platform results lock is poisoned".to_string())?;
        loop {
            match self.results.try_recv() {
                Ok(result) => preserved.push_back(result),
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    return Err(
                        "Orange platform result channel disconnected during device apply".into(),
                    )
                }
            }
        }
    }
}
