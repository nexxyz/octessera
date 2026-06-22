use crate::audio_config::decode_sample_file;
use crate::midi;
use crate::samples;
use crate::samples::resolve_sample_file;
use crate::types::{MomentaryFxTargetPayload, QueuedAudioEvent, QueuedNote};
use midir::MidiInputConnection;
use playback_runtime::{
    HostAdapter, HostMessage, MidiPort, MusicalEvent as RuntimeMusicalEvent, RuntimeAudioCommand,
    RuntimePlatformEffect, RuntimeStoreResult, SampleEntry,
};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

pub(crate) struct DesktopPlaybackHostAdapter {
    pub(crate) trigger_tx: Sender<QueuedAudioEvent>,
    pub(crate) sample_cache: Arc<Mutex<HashMap<String, realtime_engine::synth::SampleBuffer>>>,
    pub(crate) midi_out: Arc<Mutex<Option<midir::MidiOutputConnection>>>,
    pub(crate) midi_in: Arc<Mutex<Option<MidiInputConnection<()>>>>,
    pub(crate) midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    pub(crate) store_dir: PathBuf,
    pending_default_save: Option<(serde_json::Value, Instant)>,
    selected_midi_output_id: Option<String>,
    selected_midi_input_id: Option<String>,
    shutdown_requested: bool,
}

impl DesktopPlaybackHostAdapter {
    pub(crate) fn new(
        trigger_tx: Sender<QueuedAudioEvent>,
        sample_cache: Arc<Mutex<HashMap<String, realtime_engine::synth::SampleBuffer>>>,
        midi_out: Arc<Mutex<Option<midir::MidiOutputConnection>>>,
        midi_in: Arc<Mutex<Option<MidiInputConnection<()>>>>,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
        store_dir: PathBuf,
    ) -> Self {
        Self {
            trigger_tx,
            sample_cache,
            midi_out,
            midi_in,
            midi_in_handler,
            store_dir,
            pending_default_save: None,
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
        let Some((_, due_at)) = self.pending_default_save.as_ref() else {
            return Ok(Vec::new());
        };
        if Instant::now() < *due_at {
            return Ok(Vec::new());
        }
        let Some((payload, _)) = self.pending_default_save.take() else {
            return Ok(Vec::new());
        };
        self.save_default_payload(&payload)?;
        Ok(vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: Some(true),
            },
        }])
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

    fn save_default_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        let content = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
        std::fs::write(self.store_dir.join("default.json"), content).map_err(|e| e.to_string())
    }

    fn midi_ports(ports: Vec<midi::MidiPortInfo>) -> Vec<MidiPort> {
        ports
            .into_iter()
            .map(|port| MidiPort {
                id: port.id,
                name: port.name,
            })
            .collect()
    }

    fn sample_entries(entries: Vec<samples::SampleEntry>) -> Vec<SampleEntry> {
        entries
            .into_iter()
            .map(|entry| SampleEntry {
                name: entry.name,
                path: entry.path,
                is_dir: entry.is_dir,
            })
            .collect()
    }
}

impl HostAdapter for DesktopPlaybackHostAdapter {
    fn handle_musical_event(&mut self, event: &RuntimeMusicalEvent) -> Result<(), String> {
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
        self.trigger_tx
            .send(queued)
            .map_err(|e| format!("audio queue send failed: {e}"))
    }

    fn handle_platform_effect(
        &mut self,
        effect: &RuntimePlatformEffect,
    ) -> Result<Vec<HostMessage>, String> {
        match effect {
            RuntimePlatformEffect::StoreListPresets => {
                let presets_dir = self.store_dir.join("presets");
                let mut names: Vec<String> = Vec::new();
                if presets_dir.is_dir() {
                    for entry in std::fs::read_dir(&presets_dir).map_err(|e| e.to_string())? {
                        let entry = entry.map_err(|e| e.to_string())?;
                        if entry.path().extension().is_some_and(|ext| ext == "json") {
                            if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                                names.push(stem.to_string());
                            }
                        }
                    }
                }
                names.sort();
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::ListPresetsResult { names },
                }])
            }
            RuntimePlatformEffect::StoreLoadPreset { name } => {
                let path = self.store_dir.join("presets").join(format!("{name}.json"));
                let payload = if path.is_file() {
                    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    serde_json::from_str(&content).ok()
                } else {
                    None
                };
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
                let presets_dir = self.store_dir.join("presets");
                std::fs::create_dir_all(&presets_dir).map_err(|e| e.to_string())?;
                let path = presets_dir.join(format!("{name}.json"));
                let content = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
                std::fs::write(&path, content).map_err(|e| e.to_string())?;
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SavePresetResult {
                        name: name.clone(),
                        outcome: "created".to_string(),
                    },
                }])
            }
            RuntimePlatformEffect::StoreDeletePreset { name } => {
                let path = self.store_dir.join("presets").join(format!("{name}.json"));
                let ok = path.is_file() && std::fs::remove_file(&path).is_ok();
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::DeletePresetResult {
                        name: name.clone(),
                        ok,
                    },
                }])
            }
            RuntimePlatformEffect::StoreLoadDefault => {
                self.pending_default_save = None;
                let path = self.store_dir.join("default.json");
                let payload = if path.is_file() {
                    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    match serde_json::from_str(&content) {
                        Ok(payload) => Some(payload),
                        Err(error) => {
                            return Ok(vec![HostMessage::RuntimeResult {
                                result: RuntimeStoreResult::StoreError {
                                    message: format!("Default load failed: {error}"),
                                },
                            }]);
                        }
                    }
                } else {
                    None
                };
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::LoadDefaultResult { payload },
                }])
            }
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                if mode.as_deref() == Some("deferred") {
                    self.pending_default_save = Some((
                        payload.clone(),
                        Instant::now() + Duration::from_millis(DEFERRED_DEFAULT_SAVE_MS),
                    ));
                    return Ok(vec![]);
                }
                self.pending_default_save = None;
                self.save_default_payload(payload)?;
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SaveDefaultResult {
                        ok: true,
                        is_auto: None,
                    },
                }])
            }
            RuntimePlatformEffect::AudioCommand { command } => {
                self.handle_audio_command(command)?;
                Ok(vec![])
            }
            RuntimePlatformEffect::MidiListOutputsRequest => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::MidiListOutputsResult {
                    outputs: Self::midi_ports(midi::list_outputs()?),
                },
            }]),
            RuntimePlatformEffect::MidiListInputsRequest => Ok(vec![HostMessage::RuntimeResult {
                result: RuntimeStoreResult::MidiListInputsResult {
                    inputs: Self::midi_ports(midi::list_inputs()?),
                },
            }]),
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
                self.handle_midi_message(&[0xFC])?;
                for channel in 0..16_u8 {
                    self.handle_midi_message(&[0xB0 | channel, 120, 0])?;
                    self.handle_midi_message(&[0xB0 | channel, 123, 0])?;
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
            RuntimePlatformEffect::Shutdown => {
                self.shutdown_requested = true;
                Ok(vec![])
            }
            RuntimePlatformEffect::HardwareTest
            | RuntimePlatformEffect::UpdateCheck
            | RuntimePlatformEffect::UpdateApply
            | RuntimePlatformEffect::Rollback => Ok(vec![]),
            RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            } => match samples::sample_list(dir.clone()) {
                Ok(entries) => Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SampleListResult {
                        instrument_slot: *instrument_slot,
                        sample_slot: *sample_slot,
                        dir: dir.clone(),
                        entries: Self::sample_entries(entries),
                    },
                }]),
                Err(message) => Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SampleListError {
                        instrument_slot: *instrument_slot,
                        sample_slot: *sample_slot,
                        dir: dir.clone(),
                        message,
                    },
                }]),
            },
        }
    }

    fn handle_audio_command(&mut self, command: &RuntimeAudioCommand) -> Result<(), String> {
        let event = match command {
            RuntimeAudioCommand::MomentaryFxStart {
                id,
                fx_type,
                params,
                target,
            } => QueuedAudioEvent::MomentaryFxStart {
                id: id.clone(),
                fx_type: fx_type.clone(),
                params: params.clone(),
                target: match target {
                    playback_runtime::RuntimeMomentaryFxTarget::Global => {
                        MomentaryFxTargetPayload::Global
                    }
                    playback_runtime::RuntimeMomentaryFxTarget::FxBus { index } => {
                        MomentaryFxTargetPayload::FxBus { index: *index }
                    }
                    playback_runtime::RuntimeMomentaryFxTarget::Instrument { index } => {
                        MomentaryFxTargetPayload::Instrument { index: *index }
                    }
                },
            },
            RuntimeAudioCommand::MomentaryFxUpdate { id, params } => {
                QueuedAudioEvent::MomentaryFxUpdate {
                    id: id.clone(),
                    params: params.clone(),
                }
            }
            RuntimeAudioCommand::MomentaryFxStop { id } => {
                QueuedAudioEvent::MomentaryFxStop { id: id.clone() }
            }
            RuntimeAudioCommand::SamplePreview {
                instrument_slot,
                sample_slot: _,
                path,
                velocity,
            } => {
                let full_path =
                    resolve_sample_file(path).ok_or_else(|| "invalid sample path".to_string())?;
                let buffer = {
                    let mut cache = self
                        .sample_cache
                        .lock()
                        .map_err(|_| "sample cache poisoned".to_string())?;
                    if let Some(buffer) = cache.get(&full_path).cloned() {
                        buffer
                    } else {
                        let buffer = decode_sample_file(&full_path)
                            .ok_or_else(|| "sample decode failed".to_string())?;
                        cache.insert(full_path, buffer.clone());
                        buffer
                    }
                };
                QueuedAudioEvent::PreviewSample {
                    instrument_slot: (*instrument_slot).min(INSTRUMENT_SLOT_COUNT - 1) as u8,
                    buffer,
                    velocity: *velocity,
                }
            }
        };
        self.trigger_tx
            .send(event)
            .map_err(|e| format!("audio queue send failed: {e}"))
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), String> {
        let mut guard = self
            .midi_out
            .lock()
            .map_err(|_| "midi mutex poisoned".to_string())?;
        let Some(conn) = guard.as_mut() else {
            return Ok(());
        };
        conn.send(_bytes).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[path = "host_adapter_tests.rs"]
mod host_adapter_tests;
