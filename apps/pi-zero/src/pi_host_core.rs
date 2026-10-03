use crate::bluetooth::BluetoothHandle;
use crate::keyboard_capture::KeyboardCaptureControl;
use crate::midi_host::MidiHost;
use crate::oled_frame_cache::{OledFrameCache, OledFrameCacheFault, OledFramePublication};
use crate::platform_service::{
    enqueue_job, PendingPiPersistence, PiPlatformService, PlatformJobKind,
};
use playback_runtime::{
    HostMessage, NativeHdmiMode, NativeRunner, PlaybackRuntime, RunnerMessage, RuntimeAdapterError,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::Value;
use std::time::Instant;

const RECOVERY_SAVE_MISSING: &str = "recovery save did not complete";

pub(crate) struct PiHostCore {
    pub(crate) platform_service: PiPlatformService,
    pub(crate) pending_default_save: PendingPiPersistence,
    pub(crate) midi: MidiHost,
    pub(crate) oled_frame_cache: OledFrameCache,
    pub(crate) recovery_save_status: Option<Result<(), String>>,
    keyboard_control: Option<KeyboardCaptureControl>,
    bluetooth: Option<BluetoothHandle>,
    bluetooth_enabled: Option<bool>,
}

impl PiHostCore {
    pub(crate) fn new(platform_service: PiPlatformService, midi: MidiHost) -> Self {
        Self {
            platform_service,
            pending_default_save: PendingPiPersistence::default(),
            midi,
            oled_frame_cache: OledFrameCache::default(),
            recovery_save_status: None,
            keyboard_control: None,
            bluetooth: if cfg!(test) {
                None
            } else {
                BluetoothHandle::spawn_system()
            },
            bluetooth_enabled: None,
        }
    }

    pub(crate) fn set_keyboard_capture_control(&mut self, control: KeyboardCaptureControl) {
        control.observe_bluetooth(self.bluetooth_enabled.unwrap_or(false));
        self.keyboard_control = Some(control);
    }

    pub(crate) fn observe_keyboard_capture_snapshot(&self, snapshot: &Value) {
        if let Some(control) = &self.keyboard_control {
            control.observe_snapshot(snapshot);
        }
    }

    /// Follows the System setting; only a change reaches the worker and keyboard capture.
    pub(crate) fn observe_bluetooth_enabled(&mut self, enabled: bool) {
        if self.bluetooth_enabled.replace(enabled) == Some(enabled) {
            return;
        }
        if let Some(control) = &self.keyboard_control {
            control.observe_bluetooth(enabled);
        }
        if let Some(bluetooth) = &self.bluetooth {
            bluetooth.set_enabled(enabled);
        }
    }

    pub(crate) fn observe_keyboard_capture_mode(&self, mode: NativeHdmiMode) {
        if let Some(control) = &self.keyboard_control {
            control.observe_hdmi_mode(mode);
        }
    }

    pub(crate) fn ingest_oled_frame(&mut self, message: &RunnerMessage) {
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

    pub(crate) fn oled_frame_fault(&self) -> Option<OledFrameCacheFault> {
        self.oled_frame_cache.fault()
    }

    pub(crate) fn handle_transfer_input(&self, message: &HostMessage) -> bool {
        if let HostMessage::DeviceInput { input, .. } = message {
            return self.platform_service.handle_transfer_input(input);
        }
        true
    }

    pub(crate) fn take_transfer_status(&mut self) -> Option<HostMessage> {
        self.pending_default_save.cancel_if_invalid(
            self.platform_service.store_write_generation(),
            self.platform_service.store_writes_blocked(),
        );
        self.platform_service.take_transfer_status()
    }

    pub(crate) fn finished_platform_results(
        &self,
        runner: &mut NativeRunner,
        max_results: usize,
    ) -> Vec<HostMessage> {
        let bluetooth = self
            .bluetooth
            .iter()
            .flat_map(BluetoothHandle::drain_status);
        self.platform_service
            .drain_platform_results(max_results)
            .into_iter()
            .filter_map(|result| {
                crate::platform_service::platform_native_persistence::finish_platform_result(
                    &self.platform_service,
                    runner,
                    result,
                )
            })
            .chain(bluetooth)
            .collect()
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

    pub(crate) fn recovery_save_ready(&self) -> Result<(), String> {
        self.recovery_save_status
            .as_ref()
            .cloned()
            .unwrap_or_else(|| Err(RECOVERY_SAVE_MISSING.into()))
    }

    pub(crate) fn take_recovery_save_status(&mut self) -> Result<(), String> {
        self.recovery_save_status
            .take()
            .unwrap_or_else(|| Err(RECOVERY_SAVE_MISSING.into()))
    }

    pub(crate) fn send_midi(&mut self, bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        self.midi
            .send(bytes)
            .map_err(RuntimeAdapterError::operation_failed)
    }

    pub(crate) fn panic_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        self.midi
            .panic()
            .map_err(RuntimeAdapterError::operation_failed)
    }

    pub(crate) fn midi_panic_status(&mut self) -> RuntimeStoreResult {
        let result = self.midi.panic();
        RuntimeStoreResult::MidiStatus {
            ok: result.is_ok(),
            message: Some(result.err().unwrap_or_else(|| "Panic sent".into())),
            selected_out_id: self.midi.selected_output_id(),
            selected_in_id: self.midi.selected_input_id(),
        }
    }

    pub(crate) fn handle_store_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Option<Vec<HostMessage>> {
        if self
            .bluetooth
            .as_ref()
            .is_some_and(|bluetooth| bluetooth.handle_effect(&request.effect))
        {
            return Some(Vec::new());
        }
        let result = match &request.effect {
            RuntimePlatformEffect::StoreLoadDefault => {
                if self.pending_default_save.has_default_pending()
                    && !self.platform_service.store_writes_blocked()
                {
                    pending_save_failure(request)
                } else {
                    store_result(
                        request,
                        self.platform_service.load_default_now(),
                        |payload| RuntimeStoreResult::LoadDefaultResult { payload },
                    )
                }
            }
            RuntimePlatformEffect::StoreLoadPreset { name } => {
                if self.pending_default_save.has_default_pending() {
                    pending_save_failure(request)
                } else {
                    store_result(
                        request,
                        self.platform_service.load_preset_now(name),
                        |payload| RuntimeStoreResult::LoadPresetResult {
                            payload,
                            name: name.clone(),
                        },
                    )
                }
            }
            RuntimePlatformEffect::StoreLoadSystem => store_result(
                request,
                self.platform_service.load_system_now(),
                |payload| RuntimeStoreResult::LoadSystemResult { payload },
            ),
            RuntimePlatformEffect::StoreSaveSystem { payload } => {
                return Some(enqueue_job(
                    &self.platform_service,
                    request,
                    PlatformJobKind::SaveSystem {
                        payload: payload.clone(),
                    },
                    "Save system".into(),
                ));
            }
            RuntimePlatformEffect::StoreSaveDefault { payload, .. } => {
                if self.platform_service.store_writes_blocked() {
                    return Some(vec![failure_message(
                        request,
                        "Save default blocked while restore awaits restored-state acknowledgement"
                            .into(),
                    )]);
                }
                self.pending_default_save.cancel();
                return Some(enqueue_job(
                    &self.platform_service,
                    request,
                    PlatformJobKind::SaveDefault {
                        payload: payload.clone(),
                        is_auto: None,
                    },
                    "Save default".into(),
                ));
            }
            RuntimePlatformEffect::StoreSaveRecovery { payload } => {
                let result = self
                    .platform_service
                    .save_recovery_now(payload)
                    .map_err(|error| format!("Save recovery failed: {error}"));
                self.recovery_save_status = Some(result.clone());
                match result {
                    Ok(()) => RuntimeStoreResult::SaveRecoveryResult { ok: true },
                    Err(error) => {
                        eprintln!("recovery save failed: {error}");
                        return Some(vec![failure_message(request, error)]);
                    }
                }
            }
            RuntimePlatformEffect::UsbSdTransferStop => {
                return Some(enqueue_job(
                    &self.platform_service,
                    request,
                    PlatformJobKind::UsbSdTransferStop,
                    "USB SD2 transfer stop".into(),
                ));
            }
            _ => return None,
        };
        Some(vec![HostMessage::RuntimeResult { result }])
    }
}

pub(crate) fn shutdown_pending_messages(request: &RuntimePlatformRequest) -> Vec<HostMessage> {
    match &request.effect {
        RuntimePlatformEffect::Reboot
        | RuntimePlatformEffect::Shutdown
        | RuntimePlatformEffect::StoreSaveDefault { .. }
        | RuntimePlatformEffect::ApplyDeviceConfigReboot { .. } => vec![failure_message(
            request,
            "shutdown request is already pending".into(),
        )],
        _ => Vec::new(),
    }
}

pub(crate) fn power_save_result(
    recovery: Result<(), String>,
    recording: Result<(), String>,
) -> Result<(), String> {
    match (recovery, recording) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(recovery), Ok(())) => Err(recovery),
        (Ok(()), Err(recording)) => Err(format!("recording stop failed: {recording}")),
        (Err(recovery), Err(recording)) => {
            Err(format!("{recovery}; recording stop failed: {recording}"))
        }
    }
}

pub(crate) fn failure_message(request: &RuntimePlatformRequest, message: String) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure {
            error: request.failure_facts(message),
        },
    }
}

pub(crate) fn unsupported_messages(request: &RuntimePlatformRequest) -> Vec<HostMessage> {
    vec![HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure {
            error: request.unsupported_facts("unsupported on this board".into()),
        },
    }]
}

fn pending_save_failure(request: &RuntimePlatformRequest) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: request.failure_facts("Save pending, try again".into()),
    }
}

fn store_result<T>(
    request: &RuntimePlatformRequest,
    loaded: Result<T, String>,
    success: impl FnOnce(T) -> RuntimeStoreResult,
) -> RuntimeStoreResult {
    match loaded {
        Ok(value) => success(value),
        Err(message) => RuntimeStoreResult::RuntimeFailure {
            error: request.failure_facts(message),
        },
    }
}
