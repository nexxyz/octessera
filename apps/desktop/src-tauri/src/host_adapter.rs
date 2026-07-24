mod audio_config_apply;
mod host_adapter_audio;
mod host_adapter_store;

use crate::audio_prep_service::DesktopAudioControl;
use crate::desktop_platform_service::{
    shape_service_unavailable_result, DesktopPlatformServiceKind, DesktopPlatformServiceRequest,
};
use crate::midi;
use crate::types::{QueuedAudioEvent, QueuedNote};
use midir::MidiInputConnection;
use playback_runtime::{
    HostAdapter, HostMessage, MusicalEvent as RuntimeMusicalEvent, RuntimeAdapterError,
    RuntimeAudioCommand, RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult,
};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const RELEASES_URL: &str = "https://github.com/nexxyz/octessera/releases";

pub(crate) struct DesktopPlaybackHostAdapter {
    pub(crate) audio: DesktopHostAudioState,
    pub(crate) midi_out: Arc<Mutex<Option<midir::MidiOutputConnection>>>,
    pub(crate) midi_in: Arc<Mutex<Option<MidiInputConnection<()>>>>,
    pub(crate) midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    pub(crate) store_dir: PathBuf,
    pending_default_save: Option<(serde_json::Value, Instant, RuntimePlatformRequest)>,
    platform_service_tx: Sender<DesktopPlatformServiceRequest>,
    selected_midi_output_id: Option<String>,
    selected_midi_input_id: Option<String>,
    shutdown_requested: bool,
}

#[derive(Clone)]
pub(crate) struct DesktopHostAudioState {
    pub(crate) trigger_tx: Sender<QueuedAudioEvent>,
    pub(crate) audio_control: DesktopAudioControl,
    pub(crate) sample_cache: Arc<Mutex<HashMap<String, realtime_engine::synth::SampleBuffer>>>,
}

impl DesktopPlaybackHostAdapter {
    pub(crate) fn new(
        audio: DesktopHostAudioState,
        midi_out: Arc<Mutex<Option<midir::MidiOutputConnection>>>,
        midi_in: Arc<Mutex<Option<MidiInputConnection<()>>>>,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        store_dir: PathBuf,
        platform_service_tx: Sender<DesktopPlatformServiceRequest>,
    ) -> Self {
        Self {
            audio,
            midi_out,
            midi_in,
            midi_in_handler,
            store_dir,
            pending_default_save: None,
            platform_service_tx,
            selected_midi_output_id: None,
            selected_midi_input_id: None,
            shutdown_requested: false,
        }
    }

    pub(crate) fn take_shutdown_request(&mut self) -> bool {
        let requested = self.shutdown_requested;
        self.shutdown_requested = false;
        requested
    }

    pub(crate) fn flush_due_default_save(&mut self) -> Result<Vec<HostMessage>, String> {
        let Some((_, due_at, _)) = self.pending_default_save.as_ref() else {
            return Ok(Vec::new());
        };
        if Instant::now() < *due_at {
            return Ok(Vec::new());
        }
        let Some((payload, _, request)) = self.pending_default_save.take() else {
            return Ok(Vec::new());
        };
        if let Err(error) = self.save_default_payload(&payload) {
            self.pending_default_save =
                Some((payload, Instant::now() + Duration::from_secs(1), request));
            return Err(error);
        }
        Ok(vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            }
            .with_identity(request.request_id, request.revision),
        }])
    }

    pub(crate) fn flush_pending_default_save_now(&mut self) -> Result<(), String> {
        let Some((payload, _, request)) = self.pending_default_save.take() else {
            return Ok(());
        };
        if let Err(error) = self.save_default_payload(&payload) {
            self.pending_default_save =
                Some((payload, Instant::now() + Duration::from_secs(1), request));
            return Err(error);
        }
        Ok(())
    }

    fn queued_note(
        channel: &u8,
        note: &u8,
        velocity: &u8,
        duration_ms: &Option<u32>,
    ) -> QueuedAudioEvent {
        QueuedAudioEvent::Note(QueuedNote {
            instrument_slot: (*channel).clamp(0, (INSTRUMENT_SLOT_COUNT - 1) as u8),
            note: (*note).min(127),
            velocity: (*velocity).clamp(1, 127),
            duration_ms: duration_ms.unwrap_or(86_400_000).clamp(10, 86_400_000),
        })
    }

    fn enqueue_platform_service_request(
        &self,
        runtime_request: &RuntimePlatformRequest,
        kind: DesktopPlatformServiceKind,
    ) -> Vec<HostMessage> {
        let request = DesktopPlatformServiceRequest::new(runtime_request.clone(), kind);
        match self.platform_service_tx.send(request) {
            Ok(()) => Vec::new(),
            Err(error) => shape_service_unavailable_result(
                error.0,
                "Desktop platform service unavailable".into(),
            ),
        }
    }
}

impl HostAdapter for DesktopPlaybackHostAdapter {
    fn handle_musical_event(
        &mut self,
        event: &RuntimeMusicalEvent,
    ) -> Result<(), RuntimeAdapterError> {
        let queued = match event {
            RuntimeMusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => Self::queued_note(channel, note, velocity, duration_ms),
            RuntimeMusicalEvent::NoteOff { channel, note } => QueuedAudioEvent::NoteOff {
                instrument_slot: (*channel).clamp(0, (INSTRUMENT_SLOT_COUNT - 1) as u8),
                note: (*note).min(127),
            },
            RuntimeMusicalEvent::Cc {
                channel,
                controller,
                value,
            } => QueuedAudioEvent::Cc {
                instrument_slot: (*channel).clamp(0, (INSTRUMENT_SLOT_COUNT - 1) as u8),
                controller: (*controller).min(127),
                value: (*value).min(127),
            },
        };
        Ok(self
            .audio
            .trigger_tx
            .send(queued)
            .map_err(|e| format!("audio queue send failed: {e}"))?)
    }

    fn handle_platform_effect(
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
                let payload = self.load_preset_payload(name)?;
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::LoadPresetResult {
                        name: name.clone(),
                        payload,
                    },
                }])
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
            RuntimePlatformEffect::StoreLoadDefault => Ok(self.load_default_result()?),
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                Ok(self.save_default_result(request, payload, mode.as_deref())?)
            }
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
            RuntimePlatformEffect::UsbApplyReboot { payload } => {
                self.save_default_result(request, payload, Some("overwrite"))?;
                self.shutdown_requested = true;
                Ok(vec![])
            }
            RuntimePlatformEffect::RecordingStartAudio { .. }
            | RuntimePlatformEffect::RecordingStop => {
                println!("SD audio recording is unsupported on desktop host");
                Ok(vec![])
            }
            RuntimePlatformEffect::UsbSdTransferStart
            | RuntimePlatformEffect::UsbSdTransferStop => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::StoreError {
                    message: "USB SD2 transfer is Pi-only".into(),
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
            RuntimePlatformEffect::MidiSelectOutput { id } => {
                let result = midi::select_output(id.clone(), &self.midi_out);
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
                self.shutdown_requested = true;
                Ok(vec![])
            }
            RuntimePlatformEffect::HardwareTest => Ok(vec![]),
            RuntimePlatformEffect::UpdateCheck => Ok(vec![HostMessage::RuntimeResult {
                result: open_releases_page(),
            }]),
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

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        self.handle_runtime_audio_command(command)
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        let mut guard = self
            .midi_out
            .lock()
            .map_err(|_| "midi mutex poisoned".to_string())?;
        let Some(conn) = guard.as_mut() else {
            return Ok(());
        };
        Ok(conn.send(_bytes).map_err(|e| e.to_string())?)
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        self.audio
            .trigger_tx
            .send(crate::types::QueuedAudioEvent::AllNotesOff)
            .map_err(|error| RuntimeAdapterError::from(error.to_string()))
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        let mut first_error = None;
        for bytes in std::iter::once(vec![0xFC]).chain(
            (0..16_u8)
                .flat_map(|channel| [vec![0xB0 | channel, 120, 0], vec![0xB0 | channel, 123, 0]]),
        ) {
            if let Err(error) = self.handle_midi_message(&bytes) {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
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

#[cfg(test)]
#[path = "host_adapter_tests.rs"]
mod host_adapter_tests;
