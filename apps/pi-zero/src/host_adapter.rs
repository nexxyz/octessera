#[path = "host_adapter_midi.rs"]
mod host_adapter_midi;
#[path = "host_adapter_store.rs"]
mod host_adapter_store;

use crate::audio::AudioService;
use crate::host_audio_command::send_audio_command;
use crate::platform_service::{PiPlatformService, PlatformJob};
use midir::{MidiInputConnection, MidiOutputConnection};
use playback_runtime::{
    HostAdapter, HostMessage, MusicalEvent as RuntimeMusicalEvent, RuntimeAudioCommand,
    RuntimePlatformEffect, RuntimeStoreResult,
};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use rodio_engine_source::EngineEvent;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub struct PiPlaybackHostAdapter {
    audio: Option<AudioService>,
    store_dir: PathBuf,
    samples_dir: PathBuf,
    platform_service: PiPlatformService,
    pending_default_save: Option<(serde_json::Value, Instant)>,
    midi_out: Option<MidiOutputConnection>,
    midi_in: Option<MidiInputConnection<()>>,
    midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    selected_midi_output_id: Option<String>,
    selected_midi_input_id: Option<String>,
    power_request: Option<PiPowerRequest>,
    latest_recovery_payload: Option<serde_json::Value>,
}

#[derive(Clone, Copy)]
pub enum PiPowerRequest {
    Reboot,
    Shutdown,
}

impl PiPlaybackHostAdapter {
    pub fn new(
        audio: Option<AudioService>,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    ) -> Self {
        let platform_service = PiPlatformService::new(store_dir.clone(), samples_dir.clone());
        Self {
            audio,
            store_dir,
            samples_dir,
            platform_service,
            pending_default_save: None,
            midi_out: None,
            midi_in: None,
            midi_in_handler,
            selected_midi_output_id: None,
            selected_midi_input_id: None,
            power_request: None,
            latest_recovery_payload: None,
        }
    }

    pub fn take_power_request(&mut self) -> Option<PiPowerRequest> {
        self.power_request.take()
    }

    pub fn flush_due_default_save(&mut self) -> Result<Vec<HostMessage>, String> {
        let Some((_, due_at)) = self.pending_default_save.as_ref() else {
            return Ok(Vec::new());
        };
        if Instant::now() < *due_at {
            return Ok(Vec::new());
        }
        let Some((payload, _)) = self.pending_default_save.as_ref() else {
            return Ok(Vec::new());
        };
        let payload = payload.clone();
        if let Err(message) = self.platform_service.enqueue(PlatformJob::SaveDefault {
            payload: payload.clone(),
            is_auto: Some(true),
        }) {
            self.pending_default_save = Some((payload, retry_default_save_at()));
            return Ok(vec![store_error(format!(
                "Auto-save queue failed: {message}"
            ))]);
        }
        self.pending_default_save = None;
        Ok(Vec::new())
    }

    pub fn drain_platform_results(&self, max_results: usize) -> Vec<HostMessage> {
        self.platform_service.drain_results(max_results)
    }
}

impl HostAdapter for PiPlaybackHostAdapter {
    fn handle_musical_event(&mut self, event: &RuntimeMusicalEvent) -> Result<(), String> {
        let Some(audio) = &self.audio else {
            return Ok(());
        };
        match event {
            RuntimeMusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => audio.send_realtime(EngineEvent::NoteOn {
                instrument_slot: (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                note: (*note).min(127),
                velocity: (*velocity).clamp(1, 127),
                duration_ms: duration_ms.unwrap_or(86_400_000).clamp(10, 86_400_000),
            }),
            RuntimeMusicalEvent::NoteOff { channel, note } => {
                audio.send_realtime(EngineEvent::NoteOff {
                    instrument_slot: (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                    note: (*note).min(127),
                })
            }
            RuntimeMusicalEvent::Cc {
                channel,
                controller,
                value,
            } => audio.send_realtime(EngineEvent::Cc {
                instrument_slot: (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                controller: (*controller).min(127),
                value: (*value).min(127),
            }),
        }
    }

    fn handle_platform_effect(
        &mut self,
        effect: &RuntimePlatformEffect,
    ) -> Result<Vec<HostMessage>, String> {
        let result = match effect {
            RuntimePlatformEffect::StoreListPresets => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::ListPresets) {
                    return Ok(vec![store_error(format!(
                        "Preset list queued failed: {message}"
                    ))]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreLoadPreset { name } => {
                if let Err(message) = self
                    .platform_service
                    .enqueue(PlatformJob::LoadPreset { name: name.clone() })
                {
                    return Ok(vec![store_error(format!(
                        "Load {name} queued failed: {message}"
                    ))]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreSavePreset { name, payload, .. } => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::SavePreset {
                    name: name.clone(),
                    payload: payload.clone(),
                }) {
                    return Ok(vec![store_error(format!(
                        "Save {name} queued failed: {message}"
                    ))]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreDeletePreset { name } => {
                if let Err(message) = self
                    .platform_service
                    .enqueue(PlatformJob::DeletePreset { name: name.clone() })
                {
                    return Ok(vec![store_error(format!(
                        "Delete {name} queued failed: {message}"
                    ))]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreLoadDefault => self.load_default_result()?,
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                match self.save_default_result(payload, mode.as_deref())? {
                    Some(result) => result,
                    None => return Ok(Vec::new()),
                }
            }
            RuntimePlatformEffect::StoreSaveBackup { payload } => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::SaveBackup {
                    payload: payload.clone(),
                }) {
                    return Ok(vec![store_error(format!(
                        "Save backup queued failed: {message}"
                    ))]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::StoreSaveRecovery { payload } => {
                self.latest_recovery_payload = Some(payload.clone());
                if let Err(message) = self.platform_service.save_recovery_now(payload) {
                    return Ok(vec![store_error(format!(
                        "Save recovery failed: {message}"
                    ))]);
                }
                return Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SaveRecoveryResult { ok: true },
                }]);
            }
            RuntimePlatformEffect::MidiListOutputsRequest => {
                RuntimeStoreResult::MidiListOutputsResult {
                    outputs: Self::list_midi_outputs()?,
                }
            }
            RuntimePlatformEffect::MidiListInputsRequest => {
                RuntimeStoreResult::MidiListInputsResult {
                    inputs: Self::list_midi_inputs()?,
                }
            }
            RuntimePlatformEffect::MidiSelectOutput { id } => {
                let result = self.select_output(id.clone());
                RuntimeStoreResult::MidiStatus {
                    ok: result.is_ok(),
                    message: result.err(),
                    selected_out_id: self.selected_midi_output_id.clone(),
                    selected_in_id: self.selected_midi_input_id.clone(),
                }
            }
            RuntimePlatformEffect::MidiSelectInput { id } => {
                let result = self.select_input(id.clone());
                RuntimeStoreResult::MidiStatus {
                    ok: result.is_ok(),
                    message: result.err(),
                    selected_out_id: self.selected_midi_output_id.clone(),
                    selected_in_id: self.selected_midi_input_id.clone(),
                }
            }
            RuntimePlatformEffect::MidiPanic => {
                self.handle_midi_message(&[0xFC])?;
                for channel in 0..16_u8 {
                    self.handle_midi_message(&[0xB0 | channel, 120, 0])?;
                    self.handle_midi_message(&[0xB0 | channel, 123, 0])?;
                }
                RuntimeStoreResult::MidiStatus {
                    ok: true,
                    message: Some("Panic sent".into()),
                    selected_out_id: self.selected_midi_output_id.clone(),
                    selected_in_id: self.selected_midi_input_id.clone(),
                }
            }
            RuntimePlatformEffect::Reboot => {
                if let Some(payload) = &self.latest_recovery_payload {
                    if let Err(message) = self.platform_service.save_recovery_now(payload) {
                        return Ok(vec![store_error(format!(
                            "Save recovery failed: {message}"
                        ))]);
                    }
                }
                self.power_request = Some(PiPowerRequest::Reboot);
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::Shutdown => {
                if let Some(payload) = &self.latest_recovery_payload {
                    if let Err(message) = self.platform_service.save_recovery_now(payload) {
                        return Ok(vec![store_error(format!(
                            "Save recovery failed: {message}"
                        ))]);
                    }
                }
                self.power_request = Some(PiPowerRequest::Shutdown);
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::HardwareTest => {
                println!("system.hardwareTest requested (planned guided hardware diagnostic)");
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::UpdateCheck => {
                println!("system.updateCheck requested (placeholder)");
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::UpdateApply => {
                println!("system.updateApply requested (placeholder)");
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::Rollback => {
                println!("system.rollback requested (placeholder)");
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            } => {
                if let Err(message) = self.platform_service.enqueue(PlatformJob::ListSamples {
                    instrument_slot: *instrument_slot,
                    sample_slot: *sample_slot,
                    dir: dir.clone(),
                }) {
                    return Ok(vec![HostMessage::RuntimeResult {
                        result: RuntimeStoreResult::SampleListError {
                            instrument_slot: *instrument_slot,
                            sample_slot: *sample_slot,
                            dir: dir.clone(),
                            message: format!("Sample list queued failed: {message}"),
                        },
                    }]);
                }
                return Ok(Vec::new());
            }
            RuntimePlatformEffect::AudioCommand { command } => {
                self.handle_audio_command(command)?;
                return Ok(Vec::new());
            }
        };
        Ok(vec![HostMessage::RuntimeResult { result }])
    }

    fn handle_audio_command(&mut self, command: &RuntimeAudioCommand) -> Result<(), String> {
        send_audio_command(self.audio.clone(), command, &self.samples_dir)
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), String> {
        let Some(conn) = self.midi_out.as_mut() else {
            return Ok(());
        };
        conn.send(bytes).map_err(|e| e.to_string())
    }
}

fn retry_default_save_at() -> Instant {
    Instant::now() + std::time::Duration::from_millis(1_000)
}

fn store_error(message: String) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::StoreError { message },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_path_sanitizes_separators() {
        let adapter = PiPlaybackHostAdapter::new(
            None,
            PathBuf::from("store"),
            PathBuf::from("samples"),
            Arc::new(|_| {}),
        );
        assert!(
            crate::platform_service::preset_path(&adapter.store_dir, "bad/name")
                .to_string_lossy()
                .contains("bad_name.json")
        );
    }
}
