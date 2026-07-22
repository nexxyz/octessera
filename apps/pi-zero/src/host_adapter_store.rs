use crate::host_adapter::PiPlaybackHostAdapter;
use crate::platform_service::{load_json, PlatformJob, PlatformJobKind};
use playback_runtime::{RuntimePlatformRequest, RuntimeStoreResult};
use std::time::{Duration, Instant};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

impl PiPlaybackHostAdapter {
    pub(super) fn load_default_result(&mut self) -> Result<RuntimeStoreResult, String> {
        self.pending_default_save = None;
        Ok(RuntimeStoreResult::LoadDefaultResult {
            payload: load_json(&self.store_dir.join("default.json"))?,
        })
    }

    pub(super) fn save_default_result(
        &mut self,
        request: &RuntimePlatformRequest,
        payload: &serde_json::Value,
        mode: Option<&str>,
    ) -> Result<Option<RuntimeStoreResult>, String> {
        if mode == Some("deferred") {
            self.pending_default_save = Some((
                payload.clone(),
                deferred_default_save_due_at(),
                request.clone(),
            ));
            return Ok(None);
        }
        self.pending_default_save = None;
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
