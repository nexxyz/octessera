use crate::host_adapter::PiPlaybackHostAdapter;
use crate::platform_service::{PlatformJob, PlatformJobKind};
use playback_runtime::{
    HostMessage, NativeRunner, PlaybackRuntime, RuntimePlatformRequest, RuntimeStoreResult,
};
use std::time::Instant;

impl PiPlaybackHostAdapter {
    pub(crate) fn flush_native_persistence_at(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        now: Instant,
    ) -> Vec<HostMessage> {
        crate::platform_service::platform_native_autosave::flush_due_native_persistence(
            &mut self.pending_default_save,
            &self.platform_service,
            playback,
            runner,
            now,
        )
    }

    pub(crate) fn take_manual_save(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
    ) -> Option<HostMessage> {
        crate::platform_service::platform_native_autosave::take_manual_save(
            &mut self.pending_default_save,
            &self.platform_service,
            playback,
            runner,
        )
    }

    pub(crate) fn drain_platform_results_for_runner(
        &self,
        runner: &mut NativeRunner,
        max_results: usize,
    ) -> Vec<HostMessage> {
        let mut results = self
            .platform_service
            .drain_platform_results(max_results)
            .into_iter()
            .filter_map(|result| {
                crate::platform_service::platform_native_persistence::finish_platform_result(
                    &self.platform_service,
                    runner,
                    result,
                )
            })
            .collect::<Vec<_>>();
        if results.len() < max_results {
            if let Some(audio) = &self.audio {
                results.extend(audio.drain_prep_results(max_results - results.len()));
            }
        }
        results
    }

    pub(super) fn load_default_result(&mut self) -> Result<RuntimeStoreResult, String> {
        self.pending_default_save.cancel();
        let payload = self.platform_service.load_default_now()?;
        Ok(RuntimeStoreResult::LoadDefaultResult { payload })
    }

    pub(super) fn save_default_result(
        &mut self,
        request: &RuntimePlatformRequest,
        payload: &serde_json::Value,
        _mode: Option<&str>,
    ) -> Result<Option<RuntimeStoreResult>, String> {
        if let Err(message) = crate::usb_config_validation::validate_raspberry_usb_payload(payload)
        {
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
        self.pending_default_save.cancel();
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
