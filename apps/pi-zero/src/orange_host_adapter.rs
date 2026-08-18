use crate::audio::AudioService;
use crate::main_paths::{default_samples_dir, default_store_dir};
use crate::midi_host::{MidiHost, RuntimeOutputSink};
use crate::oled_frame_cache::{OledFrameCache, OledFramePublication};
use crate::orange_audio::OrangeAudioHost;
use crate::orange_device_apply::OrangeShutdownRequest;
use crate::platform_service::{load_json, PiPlatformService, PlatformJob, PlatformJobKind};
use crate::setup_portal::start_failure_message;
use playback_runtime::{
    DeferredDefaultSave, HostAdapter, HostMessage, MusicalEvent, RuntimeAdapterError,
    RuntimeAudioCommand, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

pub(crate) struct OrangeHostAdapter {
    audio: AudioService,
    audio_host: OrangeAudioHost,
    platform_service: PiPlatformService,
    store_dir: PathBuf,
    pending_default_save: DeferredDefaultSave,
    midi: MidiHost,
    oled_frame_cache: OledFrameCache,
    shutdown_request: Option<OrangeShutdownRequest>,
}

impl OrangeHostAdapter {
    pub(crate) fn handle_transfer_input(&self, message: &HostMessage) {
        if let HostMessage::DeviceInput { input, .. } = message {
            self.platform_service.handle_transfer_input(input);
        }
    }

    pub(crate) fn audio_service(&self) -> AudioService {
        self.audio.clone()
    }

    pub(crate) fn shutdown_pending(&self) -> bool {
        self.shutdown_request.is_some()
    }

    pub(crate) fn take_shutdown_request(&mut self) -> Option<OrangeShutdownRequest> {
        self.shutdown_request.take()
    }
    pub(crate) fn new(
        audio: AudioService,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
    ) -> Result<Self, String> {
        Self::with_directories(
            audio,
            default_store_dir(),
            default_samples_dir(),
            midi_in_handler,
            usb_midi_out_enabled,
        )
    }

    pub(crate) fn with_directories(
        audio: AudioService,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
    ) -> Result<Self, String> {
        let store_dir = prepare_directory(&store_dir, "Orange store")?;
        let samples_dir = prepare_directory(&samples_dir, "Orange samples")?;
        Ok(Self {
            audio: audio.clone(),
            audio_host: OrangeAudioHost::new(audio, samples_dir.clone()),
            platform_service: PiPlatformService::new(store_dir.clone(), samples_dir),
            store_dir,
            pending_default_save: DeferredDefaultSave::default(),
            midi: MidiHost::new(midi_in_handler, usb_midi_out_enabled),
            oled_frame_cache: OledFrameCache::default(),
            shutdown_request: None,
        })
    }

    pub(crate) fn ingest_oled_frame(&mut self, message: &playback_runtime::RunnerMessage) {
        self.oled_frame_cache.ingest(message);
    }

    pub(crate) fn accept_oled_frame_reference(&mut self, snapshot: &Value) {
        let _ = self.oled_frame_cache.accept_reference_value(snapshot);
    }

    pub(crate) fn oled_publication_for_snapshot(
        &mut self,
        snapshot: &Value,
        initial: bool,
    ) -> Result<OledFramePublication, String> {
        self.oled_frame_cache
            .publication_for_snapshot(snapshot, initial)
    }

    pub(crate) fn oled_frame_fault(&self) -> Option<crate::oled_frame_cache::OledFrameCacheFault> {
        self.oled_frame_cache.fault()
    }

    #[cfg(all(test, any(unix, windows)))]
    pub(crate) fn with_setup_environment(
        audio: AudioService,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        usb_midi_out_enabled: bool,
        environment: crate::setup_portal::SetupPortalEnvironment,
    ) -> Result<Self, String> {
        let store_dir = prepare_directory(&store_dir, "Orange store")?;
        let samples_dir = prepare_directory(&samples_dir, "Orange samples")?;
        Ok(Self {
            audio: audio.clone(),
            audio_host: OrangeAudioHost::new(audio, samples_dir.clone()),
            platform_service: PiPlatformService::new_with_setup_environment(
                store_dir.clone(),
                samples_dir,
                environment,
            ),
            store_dir,
            pending_default_save: DeferredDefaultSave::default(),
            midi: MidiHost::new(midi_in_handler, usb_midi_out_enabled),
            oled_frame_cache: OledFrameCache::default(),
            shutdown_request: None,
        })
    }

    pub(crate) fn drain_results(&self, max_results: usize) -> Vec<HostMessage> {
        let mut results = self.platform_service.drain_results(max_results);
        if results.len() < max_results {
            results.extend(
                self.audio
                    .drain_prep_results(max_results.saturating_sub(results.len())),
            );
        }
        results
    }

    pub(crate) fn flush_due_default_save(&mut self) -> Result<Vec<HostMessage>, String> {
        let Some(entry) = self.pending_default_save.take_due(Instant::now()) else {
            return Ok(Vec::new());
        };
        let payload = entry.payload;
        let request = entry.request;
        if let Err(message) = self.platform_service.enqueue(PlatformJob::new(
            request.clone(),
            PlatformJobKind::SaveDefault {
                payload: payload.clone(),
                is_auto: Some(true),
            },
        )) {
            self.pending_default_save.retry(
                playback_runtime::DeferredDefaultSaveEntry {
                    payload,
                    due_at: Instant::now(),
                    request: request.clone(),
                },
                retry_default_save_at(),
            );
            return Ok(vec![failure_message(
                &request,
                format!("Auto-save queue failed: {message}"),
            )]);
        }
        Ok(Vec::new())
    }
    fn enqueue(
        &self,
        request: &RuntimePlatformRequest,
        kind: PlatformJobKind,
        description: impl FnOnce(String) -> String,
    ) -> Vec<HostMessage> {
        match self
            .platform_service
            .enqueue(PlatformJob::new(request.clone(), kind))
        {
            Ok(()) => Vec::new(),
            Err(message) => vec![failure_message(request, description(message))],
        }
    }
    fn midi_status(&self, ok: bool, message: Option<String>) -> RuntimeStoreResult {
        RuntimeStoreResult::MidiStatus {
            ok,
            message,
            selected_out_id: self.midi.selected_output_id(),
            selected_in_id: self.midi.selected_input_id(),
        }
    }
    fn unsupported(&self, request: &RuntimePlatformRequest, message: &str) -> Vec<HostMessage> {
        vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure {
                error: request.unsupported_facts(message.to_string()),
            },
        }]
    }
}

impl HostAdapter for OrangeHostAdapter {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        self.audio_host.handle_musical_event(event)
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if self.shutdown_pending() {
            return match &request.effect {
                RuntimePlatformEffect::StoreSaveDefault { .. }
                | RuntimePlatformEffect::ApplyDeviceConfigReboot { .. } => {
                    Ok(vec![failure_message(
                        request,
                        "Orange shutdown request is already pending".into(),
                    )])
                }
                _ => Ok(Vec::new()),
            };
        }
        let result = match &request.effect {
            RuntimePlatformEffect::StoreListPresets => {
                return Ok(
                    self.enqueue(request, PlatformJobKind::ListPresets, |message| {
                        format!("Preset list queue failed: {message}")
                    }),
                )
            }
            RuntimePlatformEffect::StoreLoadPreset { name } => {
                return Ok(self.enqueue(
                    request,
                    PlatformJobKind::LoadPreset { name: name.clone() },
                    |message| format!("Load preset queue failed: {message}"),
                ))
            }
            RuntimePlatformEffect::StoreSavePreset { name, payload, .. } => {
                return Ok(self.enqueue(
                    request,
                    PlatformJobKind::SavePreset {
                        name: name.clone(),
                        payload: payload.clone(),
                    },
                    |message| format!("Save preset queue failed: {message}"),
                ))
            }
            RuntimePlatformEffect::StoreDeletePreset { name } => {
                return Ok(self.enqueue(
                    request,
                    PlatformJobKind::DeletePreset { name: name.clone() },
                    |message| format!("Delete preset queue failed: {message}"),
                ))
            }
            RuntimePlatformEffect::StoreLoadDefault => {
                self.pending_default_save.cancel();
                let payload = load_json(&self.store_dir.join("default.json"))
                    .map_err(RuntimeAdapterError::operation_failed)?;
                RuntimeStoreResult::LoadDefaultResult { payload }
            }
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                if self.shutdown_pending() {
                    return Ok(vec![failure_message(
                        request,
                        "Orange shutdown request is already pending".into(),
                    )]);
                }
                if mode.as_deref() == Some("deferred") {
                    self.pending_default_save.schedule(
                        payload.clone(),
                        deferred_default_save_due_at(),
                        request.clone(),
                    );
                    return Ok(Vec::new());
                }
                self.pending_default_save.cancel();
                return Ok(self.enqueue(
                    request,
                    PlatformJobKind::SaveDefault {
                        payload: payload.clone(),
                        is_auto: None,
                    },
                    |message| format!("Save default queue failed: {message}"),
                ));
            }
            RuntimePlatformEffect::StoreSaveBackup { payload } => {
                return Ok(self.enqueue(
                    request,
                    PlatformJobKind::SaveBackup {
                        payload: payload.clone(),
                    },
                    |message| format!("Save backup queue failed: {message}"),
                ))
            }
            RuntimePlatformEffect::StoreSaveRecovery { payload } => {
                self.platform_service
                    .save_recovery_now(payload)
                    .map_err(RuntimeAdapterError::operation_failed)?;
                RuntimeStoreResult::SaveRecoveryResult { ok: true }
            }
            RuntimePlatformEffect::ApplyDeviceConfigReboot { payload } => {
                if self.shutdown_pending() {
                    return Ok(vec![failure_message(
                        request,
                        "Orange shutdown request is already pending".into(),
                    )]);
                }
                self.pending_default_save.cancel();
                let transaction = self
                    .platform_service
                    .prepare_orange_device_apply(payload)
                    .map_err(RuntimeAdapterError::operation_failed)?;
                self.shutdown_request = Some(OrangeShutdownRequest::ApplyDeviceConfig(transaction));
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::Reboot => {
                if self.shutdown_pending() {
                    return Ok(vec![failure_message(
                        request,
                        "Orange shutdown request is already pending".into(),
                    )]);
                }
                self.shutdown_request = Some(OrangeShutdownRequest::Reboot);
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::Shutdown => {
                if self.shutdown_pending() {
                    return Ok(vec![failure_message(
                        request,
                        "Orange shutdown request is already pending".into(),
                    )]);
                }
                self.shutdown_request = Some(OrangeShutdownRequest::Shutdown);
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::MidiListOutputsRequest => {
                RuntimeStoreResult::MidiListOutputsResult {
                    outputs: self.midi.list_outputs()?,
                }
            }
            RuntimePlatformEffect::MidiListInputsRequest => {
                RuntimeStoreResult::MidiListInputsResult {
                    inputs: self.midi.list_inputs()?,
                }
            }
            RuntimePlatformEffect::MidiSelectOutput { id } => {
                let result = self.midi.select_output(id.clone());
                self.midi_status(result.is_ok(), result.err())
            }
            RuntimePlatformEffect::MidiSelectInput { id } => {
                let result = self.midi.select_input(id.clone());
                self.midi_status(result.is_ok(), result.err())
            }
            RuntimePlatformEffect::MidiPanic => {
                self.silence_internal_audio()?;
                let result = self.midi.panic();
                self.midi_status(result.is_ok(), result.err())
            }
            RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            } => {
                return Ok(self.enqueue(
                    request,
                    PlatformJobKind::ListSamples {
                        instrument_slot: *instrument_slot,
                        sample_slot: *sample_slot,
                        dir: dir.clone(),
                    },
                    |message| format!("Sample list queue failed: {message}"),
                ))
            }
            RuntimePlatformEffect::SystemInfoRequest => {
                return Ok(
                    self.enqueue(request, PlatformJobKind::SystemInfo, |message| {
                        format!("System info queue failed: {message}")
                    }),
                )
            }
            RuntimePlatformEffect::UpdateCheck => {
                return Ok(
                    self.enqueue(request, PlatformJobKind::UpdateCheck, |message| {
                        format!("Update check queue failed: {message}")
                    }),
                );
            }
            RuntimePlatformEffect::UpdateApply => {
                return Ok(
                    self.enqueue(request, PlatformJobKind::UpdateApply, |message| {
                        format!("Update apply queue failed: {message}")
                    }),
                );
            }
            RuntimePlatformEffect::Rollback => {
                return Ok(self.enqueue(request, PlatformJobKind::Rollback, |message| {
                    format!("Rollback queue failed: {message}")
                }));
            }
            RuntimePlatformEffect::SetupPortalOpen => {
                match self.platform_service.start_setup_portal(request) {
                    Ok(status) => return Ok(vec![status]),
                    Err(failure) => return Ok(vec![start_failure_message(request, failure)]),
                }
            }
            RuntimePlatformEffect::AudioCommand { command } => {
                self.handle_audio_command(command)?;
                return Ok(Vec::new());
            }
            _ => return Ok(self.unsupported(request, "unsupported in Orange foreground runtime")),
        };
        Ok(vec![HostMessage::RuntimeResult { result }])
    }

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        self.audio_host.handle_audio_command(command)
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        if self.shutdown_pending() {
            return Ok(());
        }
        self.midi
            .send(bytes)
            .map_err(RuntimeAdapterError::operation_failed)
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        self.audio_host.silence_internal_audio()
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.midi
            .panic()
            .map_err(RuntimeAdapterError::operation_failed)
    }
}

impl RuntimeOutputSink for OrangeHostAdapter {
    fn dispatch_output(
        &mut self,
        playback: &mut playback_runtime::PlaybackRuntime,
        runner: &mut playback_runtime::NativeRunner,
        output: playback_runtime::RuntimeIngest,
    ) -> Result<(), String> {
        crate::orange_candidate::process_runtime_output(playback, runner, self, output)
    }
}

fn prepare_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("{label} directory is not usable: {error}"))?;
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("{label} directory cannot be inspected: {error}"))?;
    if !metadata.is_dir() {
        return Err(format!(
            "{label} path is not a directory: {}",
            path.display()
        ));
    }
    path.canonicalize()
        .map_err(|error| format!("{label} directory cannot be resolved: {error}"))
}

fn deferred_default_save_due_at() -> Instant {
    Instant::now() + Duration::from_millis(DEFERRED_DEFAULT_SAVE_MS)
}

fn retry_default_save_at() -> Instant {
    Instant::now() + Duration::from_secs(1)
}

fn failure_message(request: &RuntimePlatformRequest, message: String) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure {
            error: request.failure_facts(message),
        },
    }
}

#[cfg(test)]
#[path = "orange_host_adapter_apply_tests.rs"]
mod apply_tests;
#[cfg(test)]
#[path = "orange_host_adapter_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "orange_host_adapter_update_tests.rs"]
mod update_tests;
