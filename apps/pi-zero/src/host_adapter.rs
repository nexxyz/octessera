use crate::audio::AudioManager;
use midir::{MidiInputConnection, MidiOutputConnection};
use playback_runtime::{
    HostAdapter, HostMessage, MidiPort, MusicalEvent as RuntimeMusicalEvent, RuntimeAudioCommand,
    RuntimeMomentaryFxTarget, RuntimePlatformEffect, RuntimeStoreResult, SampleEntry,
};
use realtime_engine::synth::{MomentaryFxTarget, INSTRUMENT_SLOT_COUNT};
use rodio_engine_source::EngineEvent;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEFERRED_DEFAULT_SAVE_MS: u64 = 2_000;

pub struct PiPlaybackHostAdapter<'a> {
    audio: Option<&'a AudioManager>,
    store_dir: PathBuf,
    samples_dir: PathBuf,
    pending_default_save: Option<(serde_json::Value, Instant)>,
    midi_out: Option<MidiOutputConnection>,
    midi_in: Option<MidiInputConnection<()>>,
    midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    selected_midi_output_id: Option<String>,
    selected_midi_input_id: Option<String>,
}

impl<'a> PiPlaybackHostAdapter<'a> {
    pub fn new(
        audio: Option<&'a AudioManager>,
        store_dir: PathBuf,
        samples_dir: PathBuf,
        midi_in_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    ) -> Self {
        Self {
            audio,
            store_dir,
            samples_dir,
            pending_default_save: None,
            midi_out: None,
            midi_in: None,
            midi_in_handler,
            selected_midi_output_id: None,
            selected_midi_input_id: None,
        }
    }

    pub fn flush_due_default_save(&mut self) -> Result<Vec<HostMessage>, String> {
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

    fn save_default_payload(&self, payload: &serde_json::Value) -> Result<(), String> {
        std::fs::create_dir_all(&self.store_dir).map_err(|e| e.to_string())?;
        let content = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
        std::fs::write(self.store_dir.join("default.json"), content).map_err(|e| e.to_string())
    }

    fn list_presets(&self) -> Result<Vec<String>, String> {
        let presets_dir = self.store_dir.join("presets");
        let mut names = Vec::new();
        if !presets_dir.is_dir() {
            return Ok(names);
        }
        for entry in std::fs::read_dir(&presets_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                if let Some(stem) = entry.path().file_stem().and_then(|stem| stem.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    fn preset_path(&self, name: &str) -> PathBuf {
        let safe = name.replace(['/', '\\'], "_");
        self.store_dir.join("presets").join(format!("{safe}.json"))
    }

    fn load_json(path: &Path) -> Result<Option<serde_json::Value>, String> {
        if !path.is_file() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Ok(serde_json::from_str(&content).ok())
    }

    fn save_json(path: &Path, payload: &serde_json::Value) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
        std::fs::write(path, content).map_err(|e| e.to_string())
    }

    fn midi_outputs() -> Result<(midir::MidiOutput, Vec<midir::MidiOutputPort>), String> {
        let out = midir::MidiOutput::new("cellsymphony-pi-out").map_err(|e| e.to_string())?;
        let ports = out.ports();
        Ok((out, ports))
    }

    fn midi_inputs() -> Result<(midir::MidiInput, Vec<midir::MidiInputPort>), String> {
        let input = midir::MidiInput::new("cellsymphony-pi-in").map_err(|e| e.to_string())?;
        let ports = input.ports();
        Ok((input, ports))
    }

    fn list_midi_outputs() -> Result<Vec<MidiPort>, String> {
        let (out, ports) = Self::midi_outputs()?;
        Ok(ports
            .iter()
            .enumerate()
            .map(|(index, port)| MidiPort {
                id: index.to_string(),
                name: out.port_name(port).unwrap_or_else(|_| "<unknown>".into()),
            })
            .collect())
    }

    fn list_midi_inputs() -> Result<Vec<MidiPort>, String> {
        let (input, ports) = Self::midi_inputs()?;
        Ok(ports
            .iter()
            .enumerate()
            .map(|(index, port)| MidiPort {
                id: index.to_string(),
                name: input.port_name(port).unwrap_or_else(|_| "<unknown>".into()),
            })
            .collect())
    }

    fn select_output(&mut self, id: Option<String>) -> Result<(), String> {
        self.midi_out = None;
        self.selected_midi_output_id = None;
        let Some(id) = id else {
            return Ok(());
        };
        let index = id
            .parse::<usize>()
            .map_err(|_| "invalid MIDI output id".to_string())?;
        let (out, ports) = Self::midi_outputs()?;
        let port = ports
            .get(index)
            .ok_or_else(|| "MIDI output not found".to_string())?;
        self.midi_out = Some(
            out.connect(port, "cellsymphony-pi-out")
                .map_err(|e| e.to_string())?,
        );
        self.selected_midi_output_id = Some(id);
        Ok(())
    }

    fn select_input(&mut self, id: Option<String>) -> Result<(), String> {
        self.midi_in = None;
        self.selected_midi_input_id = None;
        let Some(id) = id else {
            return Ok(());
        };
        let index = id
            .parse::<usize>()
            .map_err(|_| "invalid MIDI input id".to_string())?;
        let (mut input, ports) = Self::midi_inputs()?;
        input.ignore(midir::Ignore::None);
        let port = ports
            .get(index)
            .ok_or_else(|| "MIDI input not found".to_string())?;
        let handler = self.midi_in_handler.clone();
        self.midi_in = Some(
            input
                .connect(
                    port,
                    "cellsymphony-pi-in",
                    move |_timestamp, message, _| handler(message.to_vec()),
                    (),
                )
                .map_err(|e| e.to_string())?,
        );
        self.selected_midi_input_id = Some(id);
        Ok(())
    }

    fn sample_entries(&self, dir: &str) -> Result<Vec<SampleEntry>, String> {
        let root = self
            .samples_dir
            .canonicalize()
            .unwrap_or(self.samples_dir.clone());
        let requested = root
            .join(dir)
            .canonicalize()
            .unwrap_or_else(|_| root.join(dir));
        if !requested.starts_with(&root) {
            return Err("sample directory outside sample root".into());
        }
        if !requested.is_dir() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        if requested != root {
            entries.push(SampleEntry {
                name: "..".into(),
                path: parent_relative(&root, &requested),
                is_dir: true,
            });
        }
        for entry in std::fs::read_dir(&requested).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let is_dir = path.is_dir();
            if !is_dir && path.extension().is_none_or(|ext| ext != "wav") {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let relative = path
                .strip_prefix(&root)
                .ok()
                .and_then(|path| path.to_str())
                .unwrap_or(name)
                .replace('\\', "/");
            entries.push(SampleEntry {
                name: name.to_string(),
                path: relative,
                is_dir,
            });
        }
        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
        Ok(entries)
    }
}

impl HostAdapter for PiPlaybackHostAdapter<'_> {
    fn handle_musical_event(&mut self, event: &RuntimeMusicalEvent) -> Result<(), String> {
        let Some(audio) = self.audio else {
            return Ok(());
        };
        match event {
            RuntimeMusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => audio.send(EngineEvent::NoteOn {
                instrument_slot: (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                note: (*note).min(127),
                velocity: (*velocity).clamp(1, 127),
                duration_ms: duration_ms.unwrap_or(86_400_000).clamp(10, 86_400_000),
            }),
            RuntimeMusicalEvent::NoteOff { channel, note } => audio.send(EngineEvent::NoteOff {
                instrument_slot: (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                note: (*note).min(127),
            }),
            RuntimeMusicalEvent::Cc {
                channel,
                controller,
                value,
            } => audio.send(EngineEvent::Cc {
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
            RuntimePlatformEffect::StoreListPresets => RuntimeStoreResult::ListPresetsResult {
                names: self.list_presets()?,
            },
            RuntimePlatformEffect::StoreLoadPreset { name } => {
                RuntimeStoreResult::LoadPresetResult {
                    name: name.clone(),
                    payload: Self::load_json(&self.preset_path(name))?,
                }
            }
            RuntimePlatformEffect::StoreSavePreset { name, payload, .. } => {
                let existed = self.preset_path(name).is_file();
                Self::save_json(&self.preset_path(name), payload)?;
                RuntimeStoreResult::SavePresetResult {
                    name: name.clone(),
                    outcome: if existed { "overwritten" } else { "created" }.into(),
                }
            }
            RuntimePlatformEffect::StoreDeletePreset { name } => {
                let path = self.preset_path(name);
                RuntimeStoreResult::DeletePresetResult {
                    name: name.clone(),
                    ok: path.is_file() && std::fs::remove_file(path).is_ok(),
                }
            }
            RuntimePlatformEffect::StoreLoadDefault => {
                self.pending_default_save = None;
                RuntimeStoreResult::LoadDefaultResult {
                    payload: Self::load_json(&self.store_dir.join("default.json"))?,
                }
            }
            RuntimePlatformEffect::StoreSaveDefault { payload, mode } => {
                if mode.as_deref() == Some("deferred") {
                    self.pending_default_save = Some((
                        payload.clone(),
                        Instant::now() + Duration::from_millis(DEFERRED_DEFAULT_SAVE_MS),
                    ));
                    return Ok(Vec::new());
                }
                self.pending_default_save = None;
                self.save_default_payload(payload)?;
                RuntimeStoreResult::SaveDefaultResult {
                    ok: true,
                    is_auto: None,
                }
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
            RuntimePlatformEffect::SampleListRequest {
                instrument_slot,
                sample_slot,
                dir,
            } => match self.sample_entries(dir) {
                Ok(entries) => RuntimeStoreResult::SampleListResult {
                    instrument_slot: *instrument_slot,
                    sample_slot: *sample_slot,
                    dir: dir.clone(),
                    entries,
                },
                Err(message) => RuntimeStoreResult::SampleListError {
                    instrument_slot: *instrument_slot,
                    sample_slot: *sample_slot,
                    dir: dir.clone(),
                    message,
                },
            },
            RuntimePlatformEffect::AudioCommand { command } => {
                self.handle_audio_command(command)?;
                return Ok(Vec::new());
            }
        };
        Ok(vec![HostMessage::RuntimeResult { result }])
    }

    fn handle_audio_command(&mut self, command: &RuntimeAudioCommand) -> Result<(), String> {
        let Some(audio) = self.audio else {
            return Ok(());
        };
        match command {
            RuntimeAudioCommand::MomentaryFxStart {
                id,
                fx_type,
                params,
                target,
            } => audio.send(EngineEvent::MomentaryFxStart {
                id: id.clone(),
                fx_type: fx_type.clone(),
                params: params.clone(),
                target: match target {
                    RuntimeMomentaryFxTarget::Global => MomentaryFxTarget::Global,
                    RuntimeMomentaryFxTarget::FxBus { index } => {
                        MomentaryFxTarget::FxBus { index: *index }
                    }
                    RuntimeMomentaryFxTarget::Instrument { index } => {
                        MomentaryFxTarget::Instrument { index: *index }
                    }
                },
            }),
            RuntimeAudioCommand::MomentaryFxUpdate { id, params } => {
                audio.send(EngineEvent::MomentaryFxUpdate {
                    id: id.clone(),
                    params: params.clone(),
                })
            }
            RuntimeAudioCommand::MomentaryFxStop { id } => {
                audio.send(EngineEvent::MomentaryFxStop { id: id.clone() })
            }
            RuntimeAudioCommand::SamplePreview { .. } => Ok(()),
        }
    }

    fn handle_midi_message(&mut self, bytes: &[u8]) -> Result<(), String> {
        let Some(conn) = self.midi_out.as_mut() else {
            return Ok(());
        };
        conn.send(bytes).map_err(|e| e.to_string())
    }
}

fn parent_relative(root: &Path, requested: &Path) -> String {
    requested
        .parent()
        .unwrap_or(root)
        .strip_prefix(root)
        .ok()
        .and_then(|path| path.to_str())
        .unwrap_or("")
        .replace('\\', "/")
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
        assert!(adapter
            .preset_path("bad/name")
            .to_string_lossy()
            .contains("bad_name.json"));
    }
}
