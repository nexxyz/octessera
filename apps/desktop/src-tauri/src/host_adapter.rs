use crate::audio_config::decode_sample_file;
use crate::samples::resolve_sample_file;
use crate::types::{MomentaryFxTargetPayload, QueuedAudioEvent, QueuedNote};
use playback_runtime::{
    HostAdapter, HostMessage, MusicalEvent as RuntimeMusicalEvent, RuntimeAudioCommand,
    RuntimePlatformEffect, RuntimeStoreResult,
};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub(crate) struct DesktopPlaybackHostAdapter {
    pub(crate) trigger_tx: Sender<QueuedAudioEvent>,
    pub(crate) sample_cache: Arc<Mutex<HashMap<String, realtime_engine::synth::SampleBuffer>>>,
    pub(crate) midi_out: Arc<Mutex<Option<midir::MidiOutputConnection>>>,
    pub(crate) store_dir: PathBuf,
}

impl DesktopPlaybackHostAdapter {
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
                let path = self.store_dir.join("default.json");
                let payload = if path.is_file() {
                    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    serde_json::from_str(&content).ok()
                } else {
                    None
                };
                Ok(vec![HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::LoadDefaultResult { payload },
                }])
            }
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                if mode.as_deref() == Some("deferred") {
                    return Ok(vec![]);
                }
                let content = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
                std::fs::write(self.store_dir.join("default.json"), content)
                    .map_err(|e| e.to_string())?;
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
            _ => Ok(vec![]),
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
mod tests {
    use super::*;
    use playback_runtime::RuntimePlatformEffect;
    use std::sync::mpsc;

    #[test]
    fn platform_effect_audio_command_reaches_audio_queue() {
        let (tx, rx) = mpsc::channel();
        let mut adapter = DesktopPlaybackHostAdapter {
            trigger_tx: tx,
            sample_cache: Arc::new(Mutex::new(HashMap::new())),
            midi_out: Arc::new(Mutex::new(None)),
            store_dir: PathBuf::new(),
        };

        let follow_ups = adapter
            .handle_platform_effect(&RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::MomentaryFxStop {
                    id: "preview".into(),
                },
            })
            .unwrap();

        assert!(follow_ups.is_empty());
        assert!(matches!(
            rx.try_recv().unwrap(),
            QueuedAudioEvent::MomentaryFxStop { id } if id == "preview"
        ));
    }
}
