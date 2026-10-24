mod host_adapter_audio;
mod host_adapter_audio_validation;
mod host_adapter_drum;
mod host_adapter_musical;
mod host_adapter_platform_effects;
mod host_adapter_recording;
mod host_adapter_store;

use crate::audio_prep_service::DesktopAudioControl;
use crate::desktop_platform_service::{
    admit_platform_service_request, DesktopPlatformServiceKind, DesktopPlatformServiceRequest,
};
use crate::recording::DesktopRecording;
use host_adapter_audio::audio_queue_error;
use midir::MidiInputConnection;
use playback_runtime::{
    DeferredDefaultSave, HostAdapter, HostMessage, MusicalEvent as RuntimeMusicalEvent,
    RuntimeAdapterError, RuntimeAudioCommand, RuntimeOperation, RuntimePlatformRequest,
    RuntimeStoreResult,
};
use rodio_engine_source::{EngineEvent, EngineEventSender, QueueKind, QueueSendError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) struct DesktopPlaybackHostAdapter {
    pub(crate) audio: DesktopHostAudioState,
    pub(crate) midi_out: Arc<Mutex<Option<midir::MidiOutputConnection>>>,
    pub(crate) midi_in: Arc<Mutex<Option<MidiInputConnection<()>>>>,
    pub(crate) midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    pub(crate) store_dir: PathBuf,
    pending_default_save: DeferredDefaultSave,
    platform_service_tx: SyncSender<DesktopPlatformServiceRequest>,
    selected_midi_output_id: Option<String>,
    selected_midi_input_id: Option<String>,
    shutdown_requested: bool,
    momentary_fx_types: HashMap<String, (u64, String)>,
}

#[derive(Clone)]
pub(crate) struct DesktopHostAudioState {
    pub(crate) engine_tx: EngineEventSender,
    pub(crate) audio_control: DesktopAudioControl,
    pub(crate) recording: DesktopRecording,
}

impl DesktopPlaybackHostAdapter {
    pub(crate) fn new(
        audio: DesktopHostAudioState,
        midi_out: Arc<Mutex<Option<midir::MidiOutputConnection>>>,
        midi_in: Arc<Mutex<Option<MidiInputConnection<()>>>>,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        store_dir: PathBuf,
        platform_service_tx: SyncSender<DesktopPlatformServiceRequest>,
    ) -> Self {
        Self {
            audio,
            midi_out,
            midi_in,
            midi_in_handler,
            store_dir,
            pending_default_save: DeferredDefaultSave::default(),
            platform_service_tx,
            selected_midi_output_id: None,
            selected_midi_input_id: None,
            shutdown_requested: false,
            momentary_fx_types: HashMap::new(),
        }
    }

    pub(crate) fn take_shutdown_request(&mut self) -> bool {
        let requested = self.shutdown_requested;
        self.shutdown_requested = false;
        requested
    }

    pub(crate) fn flush_due_default_save(&mut self) -> Result<Vec<HostMessage>, String> {
        let Some(entry) = self.pending_default_save.take_due(Instant::now()) else {
            return Ok(Vec::new());
        };
        let payload = entry.payload;
        let request = entry.request;
        if let Err(error) = self.save_default_payload(&payload) {
            self.pending_default_save.retry(
                playback_runtime::DeferredDefaultSaveEntry {
                    payload,
                    due_at: Instant::now(),
                    request,
                },
                Instant::now() + Duration::from_secs(1),
            );
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
        let Some(entry) = self.pending_default_save.take_now() else {
            return Ok(());
        };
        let payload = entry.payload;
        let request = entry.request;
        if let Err(error) = self.save_default_payload(&payload) {
            self.pending_default_save.retry(
                playback_runtime::DeferredDefaultSaveEntry {
                    payload,
                    due_at: Instant::now(),
                    request,
                },
                Instant::now() + Duration::from_secs(1),
            );
            return Err(error);
        }
        Ok(())
    }

    fn enqueue_platform_service_request(
        &self,
        runtime_request: &RuntimePlatformRequest,
        kind: DesktopPlatformServiceKind,
    ) -> Vec<HostMessage> {
        let request = DesktopPlatformServiceRequest::new(runtime_request.clone(), kind);
        match admit_platform_service_request(&self.platform_service_tx, request) {
            Ok(()) => Vec::new(),
            Err(messages) => messages,
        }
    }
}

impl HostAdapter for DesktopPlaybackHostAdapter {
    fn handle_musical_event(
        &mut self,
        event: &RuntimeMusicalEvent,
    ) -> Result<(), RuntimeAdapterError> {
        self.handle_runtime_musical_event(event)
    }

    fn handle_cell_hit(
        &mut self,
        hit: &playback_runtime::CellHit,
    ) -> Result<(), RuntimeAdapterError> {
        self.handle_runtime_cell_hit(hit)
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        self.handle_runtime_platform_effect(request)
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
        self.send_engine_event(EngineEvent::AllNotesOff, RuntimeOperation::AudioCommand)
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

impl DesktopPlaybackHostAdapter {
    pub(super) fn send_engine_event(
        &self,
        event: EngineEvent,
        operation: RuntimeOperation,
    ) -> Result<(), RuntimeAdapterError> {
        self.handle_engine_send_result(self.audio.engine_tx.send(event), operation)
    }

    pub(super) fn handle_engine_send_result(
        &self,
        result: Result<(), QueueSendError>,
        operation: RuntimeOperation,
    ) -> Result<(), RuntimeAdapterError> {
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                if matches!(
                    error,
                    QueueSendError::Full {
                        queue: QueueKind::Musical | QueueKind::Structural
                    }
                ) {
                    let _ = self.audio.engine_tx.send(EngineEvent::AllNotesOff);
                }
                Err(audio_queue_error(error, operation))
            }
        }
    }
}

#[cfg(test)]
mod host_adapter_tests;
