use crate::host_adapter::PiPlaybackHostAdapter;
use crate::platform_service::{PlatformJob, PlatformJobKind};
use playback_runtime::{RuntimePlatformRequest, RuntimeStoreResult};
use std::time::{Duration, Instant};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

impl PiPlaybackHostAdapter {
    pub(super) fn load_default_result(&mut self) -> Result<RuntimeStoreResult, String> {
        self.pending_default_save.cancel();
        self.pending_default_save_generation = None;
        let payload = self.platform_service.load_default_now()?;
        Ok(RuntimeStoreResult::LoadDefaultResult { payload })
    }

    pub(super) fn save_default_result(
        &mut self,
        request: &RuntimePlatformRequest,
        payload: &serde_json::Value,
        mode: Option<&str>,
    ) -> Result<Option<RuntimeStoreResult>, String> {
        if let Err(message) = crate::usb_config::validate_pi_audio_outputs_payload(payload) {
            return Ok(Some(RuntimeStoreResult::RuntimeFailure {
                error: request.failure_facts(message),
            }));
        }
        if self.platform_service.store_writes_blocked() {
            return Ok(Some(RuntimeStoreResult::RuntimeFailure {
                error: request.failure_facts(
                    "Save default blocked while restore awaits restored-state acknowledgement"
                        .into(),
                ),
            }));
        }
        if mode == Some("deferred") {
            self.pending_default_save.schedule(
                payload.clone(),
                deferred_default_save_due_at(),
                request.clone(),
            );
            self.pending_default_save_generation =
                Some(self.platform_service.store_write_generation());
            return Ok(None);
        }
        self.pending_default_save.cancel();
        self.pending_default_save_generation = None;
        if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
            request.clone(),
            PlatformJobKind::SaveDefault {
                payload: payload.clone(),
                is_auto: None,
            },
        )) {
            return Ok(Some(RuntimeStoreResult::RuntimeFailure {
                error: request.failure_facts(format!("Save default queued failed: {message}")),
            }));
        }
        Ok(None)
    }
}

fn deferred_default_save_due_at() -> Instant {
    Instant::now() + Duration::from_millis(DEFERRED_DEFAULT_SAVE_MS)
}
