use super::{host_adapter_recording, DesktopPlaybackHostAdapter};
use crate::desktop_platform_service::DesktopPlatformServiceKind;
use crate::midi;
use playback_runtime::{
    HostAdapter, HostMessage, RuntimeAdapterError, RuntimeOperation, RuntimePlatformEffect,
    RuntimePlatformRequest, RuntimeSetupPortalErrorCode, RuntimeSetupPortalPhase,
    RuntimeSetupPortalStatus, RuntimeStoreResult, RuntimeUserDataTransferPhase,
    RuntimeUserDataTransferStatus,
};
use std::process::Command;

const RELEASES_URL: &str = "https://github.com/nexxyz/octessera/releases";

impl DesktopPlaybackHostAdapter {
    pub(super) fn handle_runtime_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        let effect = &request.effect;
        match effect {
            RuntimePlatformEffect::StoreListPresets => {
                let names = self.list_preset_names()?;
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::ListPresetsResult { names },
                }])
            }
            RuntimePlatformEffect::StoreLoadPreset { name } => {
                Ok(self.load_preset_result(request, name)?)
            }
            RuntimePlatformEffect::StoreSavePreset {
                name,
                payload,
                mode: _,
            } => {
                self.save_preset_payload(name, payload)?;
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SavePresetResult {
                        name: name.clone(),
                        outcome: "created".to_string(),
                    },
                }])
            }
            RuntimePlatformEffect::StoreDeletePreset { name } => {
                let ok = self.delete_preset_payload(name)?;
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::DeletePresetResult {
                        name: name.clone(),
                        ok,
                    },
                }])
            }
            RuntimePlatformEffect::StoreLoadDefault => Ok(self.load_default_result(request)?),
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                Ok(self.save_default_result(request, payload, mode.as_deref())?)
            }
            RuntimePlatformEffect::StoreLoadSystem => Ok(self.load_system_result(request)),
            RuntimePlatformEffect::StoreSaveSystem { payload } => Ok(self
                .enqueue_platform_service_request(
                    request,
                    DesktopPlatformServiceKind::SaveSystem {
                        store_dir: self.store_dir.clone(),
                        payload: payload.clone(),
                    },
                )),
            RuntimePlatformEffect::StoreSaveBackup { payload } => {
                self.save_backup_payload(payload)?;
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SaveBackupResult { ok: true },
                }])
            }
            RuntimePlatformEffect::StoreSaveRecovery { payload } => {
                self.save_recovery_payload(payload)?;
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SaveRecoveryResult { ok: true },
                }])
            }
            RuntimePlatformEffect::ApplyDeviceConfigReboot { payload } => {
                let recording_result =
                    host_adapter_recording::finalize_for_shutdown(&self.audio.recording, request)?;
                self.save_system_payload(payload)?;
                self.shutdown_requested = true;
                Ok(recording_result
                    .into_iter()
                    .map(|result| HostMessage::RuntimeResult { result })
                    .collect())
            }
            RuntimePlatformEffect::RecordingStartAudio { .. }
            | RuntimePlatformEffect::RecordingStartAudioOled { .. }
            | RuntimePlatformEffect::RecordingStop => Ok(host_adapter_recording::handle_effect(
                &self.audio.recording,
                request,
            )?
            .into_iter()
            .map(|result| HostMessage::RuntimeResult { result })
            .collect()),
            RuntimePlatformEffect::UsbSdTransferStart
            | RuntimePlatformEffect::UsbSdTransferStop => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::StoreError {
                    message: "USB SD2 transfer is Pi-only".into(),
                },
            }]),
            RuntimePlatformEffect::MaintenanceHold { .. } => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::StoreError {
                    message: "This setting is board-only".into(),
                },
            }]),
            RuntimePlatformEffect::AudioCommand { command } => {
                self.handle_audio_command(command)?;
                Ok(vec![])
            }
            RuntimePlatformEffect::MidiListOutputsRequest => Ok(self
                .enqueue_platform_service_request(
                    request,
                    DesktopPlatformServiceKind::MidiListOutputs,
                )),
            RuntimePlatformEffect::MidiListInputsRequest => Ok(self
                .enqueue_platform_service_request(
                    request,
                    DesktopPlatformServiceKind::MidiListInputs,
                )),
            RuntimePlatformEffect::SystemInfoRequest => Ok(self
                .enqueue_platform_service_request(request, DesktopPlatformServiceKind::SystemInfo)),
            RuntimePlatformEffect::SetupPortalOpen => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::SetupPortalStatus {
                    status: RuntimeSetupPortalStatus {
                        phase: RuntimeSetupPortalPhase::Unsupported,
                        disposition: None,
                        portal_suffix: None,
                        reboot_required: false,
                        error_code: Some(RuntimeSetupPortalErrorCode::Unsupported),
                    },
                }
                .with_identity(request.request_id.clone(), request.revision),
            }]),
            RuntimePlatformEffect::UserDataTransferOpen => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::UserDataTransferStatus {
                    status: RuntimeUserDataTransferStatus {
                        phase: RuntimeUserDataTransferPhase::Unsupported,
                        url: None,
                        code: None,
                        expires_in_seconds: None,
                    },
                }
                .with_identity(request.request_id.clone(), request.revision),
            }]),
            RuntimePlatformEffect::UserDataTransferClose => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::UserDataTransferStatus {
                    status: RuntimeUserDataTransferStatus {
                        phase: RuntimeUserDataTransferPhase::Closed,
                        url: None,
                        code: None,
                        expires_in_seconds: None,
                    },
                }
                .with_identity(request.request_id.clone(), request.revision),
            }]),
            RuntimePlatformEffect::MidiSelectOutput { id } => {
                let result = midi::select_output(id.clone(), &mut self.midi_out);
                if result.is_ok() {
                    self.selected_midi_output_id = id.clone();
                }
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::MidiStatus {
                        ok: result.is_ok(),
                        message: result.err(),
                        selected_out_id: self.selected_midi_output_id.clone(),
                        selected_in_id: self.selected_midi_input_id.clone(),
                    },
                }])
            }
            RuntimePlatformEffect::MidiSelectInput { id } => {
                let handler = self.midi_in_handler.clone();
                let result =
                    midi::select_input_with_handler(id.clone(), &self.midi_in, move |bytes| {
                        handler(bytes);
                    });
                if result.is_ok() {
                    self.selected_midi_input_id = id.clone();
                }
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::MidiStatus {
                        ok: result.is_ok(),
                        message: result.err(),
                        selected_out_id: self.selected_midi_output_id.clone(),
                        selected_in_id: self.selected_midi_input_id.clone(),
                    },
                }])
            }
            RuntimePlatformEffect::MidiPanic => {
                let audio_error = self.silence_internal_audio().err();
                let midi_error = self.panic_external_midi().err();
                if let Some(error) = audio_error.or(midi_error) {
                    return Err(error);
                }
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::MidiStatus {
                        ok: true,
                        message: Some("Panic sent".into()),
                        selected_out_id: self.selected_midi_output_id.clone(),
                        selected_in_id: self.selected_midi_input_id.clone(),
                    },
                }])
            }
            RuntimePlatformEffect::Reboot | RuntimePlatformEffect::Shutdown => {
                let recording_result =
                    host_adapter_recording::finalize_for_shutdown(&self.audio.recording, request)?;
                self.shutdown_requested = true;
                Ok(recording_result
                    .into_iter()
                    .map(|result| HostMessage::RuntimeResult { result })
                    .collect())
            }
            RuntimePlatformEffect::UpdateCheck => Ok(vec![HostMessage::RuntimeResult {
                result: open_releases_page(),
            }]),
            RuntimePlatformEffect::BluetoothScan { .. }
            | RuntimePlatformEffect::BluetoothDevice { .. } => {
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::RuntimeFailure {
                        error: request.unsupported_facts("Bluetooth is Pi-only".into()),
                    },
                }])
            }
            RuntimePlatformEffect::UpdateApply => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::RuntimeFailure {
                    error: request.unsupported_facts(
                        "Desktop update apply is unsupported; use the releases page".into(),
                    ),
                },
            }]),
            RuntimePlatformEffect::Rollback => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::StoreError {
                    message: "Desktop rollback is unsupported".into(),
                },
            }]),
            RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            } => Ok(self.enqueue_platform_service_request(
                request,
                DesktopPlatformServiceKind::SampleList {
                    instrument_slot: *instrument_slot,
                    sample_slot: *sample_slot,
                    dir: dir.clone(),
                },
            )),
        }
    }
}

fn open_releases_page() -> RuntimeStoreResult {
    let result = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", "", RELEASES_URL])
            .status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(RELEASES_URL).status()
    } else {
        Command::new("xdg-open").arg(RELEASES_URL).status()
    };

    match result {
        Ok(status) if status.success() => RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::RuntimeDispatch,
            request_id: None,
            revision: None,
        },
        Ok(status) => RuntimeStoreResult::StoreError {
            message: format!("Open releases page failed: {status}"),
        },
        Err(error) => RuntimeStoreResult::StoreError {
            message: format!("Open releases page failed: {error}"),
        },
    }
}
