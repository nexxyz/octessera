use crate::host_adapter::PiPlaybackHostAdapter;
use crate::platform_service::{PlatformJob, PlatformJobKind};
use playback_runtime::{
    HostMessage, NativeRunner, PlaybackRuntime, RuntimePlatformRequest, RuntimeStoreResult,
};
use std::time::Instant;

#[cfg(test)]
#[path = "host_adapter_load_admission_tests.rs"]
mod load_admission_tests;

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

    pub(super) fn load_default_result(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> RuntimeStoreResult {
        if self.pending_default_save.has_default_pending()
            && !self.platform_service.store_writes_blocked()
        {
            return pending_save_failure(request);
        }
        match self.platform_service.load_default_now() {
            Ok(payload) => RuntimeStoreResult::LoadDefaultResult { payload },
            Err(message) => RuntimeStoreResult::RuntimeFailure {
                error: request.failure_facts(message),
            },
        }
    }

    pub(super) fn load_preset_result(
        &self,
        request: &RuntimePlatformRequest,
        name: &str,
    ) -> RuntimeStoreResult {
        if self.pending_default_save.has_default_pending() {
            return pending_save_failure(request);
        }
        match self.platform_service.load_preset_now(name) {
            Ok(payload) => RuntimeStoreResult::LoadPresetResult {
                payload,
                name: name.to_string(),
            },
            Err(message) => RuntimeStoreResult::RuntimeFailure {
                error: request.failure_facts(message),
            },
        }
    }

    pub(super) fn load_patch_result(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> RuntimeStoreResult {
        match &request.effect {
            playback_runtime::RuntimePlatformEffect::StoreLoadDefault => {
                self.load_default_result(request)
            }
            playback_runtime::RuntimePlatformEffect::StoreLoadPreset { name } => {
                self.load_preset_result(request, name)
            }
            _ => unreachable!("non-patch load effect"),
        }
    }

    pub(super) fn handle_system_store_effect(
        &self,
        request: &RuntimePlatformRequest,
    ) -> Option<Vec<HostMessage>> {
        let result = match &request.effect {
            playback_runtime::RuntimePlatformEffect::StoreLoadSystem => self
                .platform_service
                .load_system_now()
                .map(|payload| RuntimeStoreResult::LoadSystemResult { payload }),
            playback_runtime::RuntimePlatformEffect::StoreSaveSystem { payload } => {
                return Some(
                    match self.platform_service.enqueue(PlatformJob::new(
                        request.clone(),
                        PlatformJobKind::SaveSystem {
                            payload: payload.clone(),
                        },
                    )) {
                        Ok(()) => Vec::new(),
                        Err(message) => vec![HostMessage::RuntimeResult {
                            result: RuntimeStoreResult::RuntimeFailure {
                                error: request
                                    .failure_facts(format!("Save system queued failed: {message}")),
                            },
                        }],
                    },
                );
            }
            _ => return None,
        };
        Some(vec![HostMessage::RuntimeResult {
            result: result.unwrap_or_else(|message| RuntimeStoreResult::RuntimeFailure {
                error: request.failure_facts(message),
            }),
        }])
    }

    pub(super) fn save_default_result(
        &mut self,
        request: &RuntimePlatformRequest,
        payload: &serde_json::Value,
        _mode: Option<&str>,
    ) -> Result<Option<RuntimeStoreResult>, String> {
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

pub(super) fn pending_save_failure(request: &RuntimePlatformRequest) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: request.failure_facts("Save pending, try again".into()),
    }
}
