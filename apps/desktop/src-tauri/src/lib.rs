mod audio_config;
mod midi;
mod samples;

use audio_config::{
    build_audio_slot_configs, decode_sample_file, parse_voice_stealing_mode, sample_bank_signature,
    sample_banks, synth_payload, AudioInstrumentsConfig, AudioRuntimePolicyConfig,
};
use midi::{midi_list_inputs, midi_list_outputs, midi_select_input, midi_select_output, midi_send};
use rodio_engine_source::{EngineEvent, EngineSource};
use samples::{resolve_sample_file, sample_list};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread;

use midir::MidiInputConnection;
use realtime_engine::synth::{INSTRUMENT_SLOT_COUNT, SAMPLE_SLOTS_PER_INSTRUMENT};
use rodio::{OutputStream, OutputStreamHandle, Sink};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use tauri::Emitter;

#[derive(Deserialize)]
#[serde(tag = "type")]
enum MusicalEventPayload {
    #[serde(rename = "note_on")]
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
        #[serde(default, rename = "durationMs")]
        duration_ms: Option<u32>,
    },
    #[serde(rename = "note_off")]
    NoteOff { channel: u8, note: u8 },
    #[serde(rename = "cc")]
    Cc {
        channel: u8,
        controller: u8,
        value: u8,
    },
    #[serde(other)]
    Unsupported,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum AudioCommandPayload {
    #[serde(rename = "momentary_fx_start")]
    MomentaryFxStart {
        id: String,
        #[serde(rename = "fxType")]
        fx_type: String,
        #[serde(default)]
        params: BTreeMap<String, Value>,
        #[serde(default)]
        target: MomentaryFxTargetPayload,
    },
    #[serde(rename = "momentary_fx_update")]
    MomentaryFxUpdate {
        id: String,
        #[serde(default)]
        params: BTreeMap<String, Value>,
    },
    #[serde(rename = "momentary_fx_stop")]
    MomentaryFxStop { id: String },
    #[serde(rename = "sample_preview")]
    SamplePreview {
        #[serde(rename = "instrumentSlot")]
        instrument_slot: usize,
        #[serde(rename = "sampleSlot")]
        sample_slot: usize,
        path: String,
        velocity: u8,
    },
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type")]
enum MomentaryFxTargetPayload {
    #[serde(rename = "global")]
    Global,
    #[serde(rename = "fx_bus")]
    FxBus { index: usize },
    #[serde(rename = "instrument")]
    Instrument { index: usize },
}

impl Default for MomentaryFxTargetPayload {
    fn default() -> Self {
        Self::Global
    }
}

struct AudioRuntime {
    _stream: OutputStream,
    handle: OutputStreamHandle,
}

#[derive(Clone, Copy, serde::Serialize)]
struct AudioLoadPayload {
    ratio: f32,
    #[serde(rename = "voiceSteal")]
    voice_steal: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct QueuedNote {
    instrument_slot: u8,
    note: u8,
    velocity: u8,
    duration_ms: u32,
}

#[derive(Clone)]
pub(crate) enum QueuedAudioEvent {
    Note(QueuedNote),
    NoteOff {
        instrument_slot: u8,
        note: u8,
    },
    Cc {
        instrument_slot: u8,
        controller: u8,
        value: u8,
    },
    PreviewSample {
        instrument_slot: u8,
        buffer: realtime_engine::synth::SampleBuffer,
        velocity: u8,
    },
    SetInstruments(realtime_engine::synth::InstrumentsConfig),
    SetSampleBanks(Vec<realtime_engine::synth::SampleBankConfig>),
    SetVoiceStealingMode(realtime_engine::synth::VoiceStealingMode),
    MomentaryFxStart {
        id: String,
        fx_type: String,
        params: BTreeMap<String, Value>,
        target: MomentaryFxTargetPayload,
    },
    MomentaryFxUpdate {
        id: String,
        params: BTreeMap<String, Value>,
    },
    MomentaryFxStop {
        id: String,
    },
}

impl AudioRuntime {
    fn new() -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| format!("audio init failed: {e}"))?;
        Ok(Self {
            _stream: stream,
            handle,
        })
    }

    fn start_engine(
        &self,
        control_rx: Receiver<EngineEvent>,
        load_tx: Sender<realtime_engine::synth::AudioLoadStatus>,
    ) -> Result<(), String> {
        let source = EngineSource::with_load_status_tx(control_rx, 48_000, Some(load_tx));
        let sink = Sink::try_new(&self.handle).map_err(|e| format!("sink create failed: {e}"))?;
        sink.append(source);
        sink.play();
        sink.detach();
        Ok(())
    }
}

pub(crate) struct AppState {
    pub(crate) trigger_tx: Sender<QueuedAudioEvent>,
    synth_slots: Mutex<[bool; INSTRUMENT_SLOT_COUNT]>,
    sample_cache: Mutex<HashMap<String, realtime_engine::synth::SampleBuffer>>,
    sample_bank_signature: Mutex<String>,
    pub(crate) midi_out: Mutex<Option<midir::MidiOutputConnection>>,
    pub(crate) midi_in: Mutex<Option<MidiInputConnection<()>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct SampleSlotConfig {
    pub(crate) slots: [Option<String>; 8],
    pub(crate) tune_semis: f32,
    pub(crate) gain_pct: f32,
    pub(crate) vel_sens_pct: f32,
}

impl Default for SampleSlotConfig {
    fn default() -> Self {
        Self {
            slots: [None, None, None, None, None, None, None, None],
            tune_semis: 0.0,
            gain_pct: 100.0,
            vel_sens_pct: 100.0,
        }
    }
}

#[tauri::command]
fn trigger_musical_event(
    event: MusicalEventPayload,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let is_synth_slot = |slot: u8| -> bool {
        if let Ok(guard) = state.synth_slots.lock() {
            return guard[(slot as usize).min(INSTRUMENT_SLOT_COUNT - 1)];
        }
        true
    };
    match event {
        MusicalEventPayload::NoteOn {
            channel,
            note,
            velocity,
            duration_ms,
        } => {
            let slot = channel.clamp(0, (INSTRUMENT_SLOT_COUNT - 1) as u8);
            let duration = duration_ms.unwrap_or(86_400_000).clamp(10, 86_400_000);
            state
                .trigger_tx
                .send(QueuedAudioEvent::Note(QueuedNote {
                    instrument_slot: slot,
                    note: note.min(127),
                    velocity: velocity.clamp(1, 127),
                    duration_ms: duration,
                }))
                .map_err(|e| format!("audio queue send failed: {e}"))
        }
        MusicalEventPayload::Cc {
            channel,
            controller,
            value,
        } => {
            let slot = channel.clamp(0, (INSTRUMENT_SLOT_COUNT - 1) as u8);
            if !is_synth_slot(slot) {
                return Ok(());
            }
            state
                .trigger_tx
                .send(QueuedAudioEvent::Cc {
                    instrument_slot: slot,
                    controller,
                    value,
                })
                .map_err(|e| format!("audio queue send failed: {e}"))
        }
        MusicalEventPayload::NoteOff { channel, note } => {
            let slot = channel.clamp(0, (INSTRUMENT_SLOT_COUNT - 1) as u8);
            state
                .trigger_tx
                .send(QueuedAudioEvent::NoteOff {
                    instrument_slot: slot,
                    note: note.min(127),
                })
                .map_err(|e| format!("audio queue send failed: {e}"))
        }
        MusicalEventPayload::Unsupported => Ok(()),
    }
}

#[tauri::command]
fn audio_set_instruments(
    config: AudioInstrumentsConfig,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let (next_slots, _) = build_audio_slot_configs(&config.instruments);
    let next_sample_signature = sample_bank_signature(&config);
    let should_update_sample_banks = {
        let mut current = state
            .sample_bank_signature
            .lock()
            .map_err(|_| "sample bank signature lock failed".to_string())?;
        if *current == next_sample_signature {
            false
        } else {
            *current = next_sample_signature;
            true
        }
    };
    let next_sample_banks = if should_update_sample_banks {
        let mut cache = state
            .sample_cache
            .lock()
            .map_err(|_| "sample cache lock failed".to_string())?;
        Some(sample_banks(&config, resolve_sample_file, |path| {
            if let Some(buffer) = cache.get(path) {
                return Some(buffer.clone());
            }
            let buffer = decode_sample_file(path)?;
            cache.insert(path.to_string(), buffer.clone());
            Some(buffer)
        }))
    } else {
        None
    };
    if let Ok(mut slots) = state.synth_slots.lock() {
        *slots = next_slots;
    }
    let synth_payload = synth_payload(&config);
    state
        .trigger_tx
        .send(QueuedAudioEvent::SetInstruments(synth_payload))
        .map_err(|e| format!("audio queue send failed: {e}"))?;
    if let Some(next_sample_banks) = next_sample_banks {
        state
            .trigger_tx
            .send(QueuedAudioEvent::SetSampleBanks(next_sample_banks))
            .map_err(|e| format!("audio queue send failed: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
fn audio_set_runtime_policy(
    policy: AudioRuntimePolicyConfig,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let mode = parse_voice_stealing_mode(&policy.voice_stealing_mode);
    state
        .trigger_tx
        .send(QueuedAudioEvent::SetVoiceStealingMode(mode))
        .map_err(|e| format!("audio queue send failed: {e}"))
}

#[tauri::command]
fn audio_command(
    command: AudioCommandPayload,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let event = match command {
        AudioCommandPayload::MomentaryFxStart {
            id,
            fx_type,
            params,
            target,
        } => QueuedAudioEvent::MomentaryFxStart {
            id,
            fx_type,
            params,
            target,
        },
        AudioCommandPayload::MomentaryFxUpdate { id, params } => {
            QueuedAudioEvent::MomentaryFxUpdate { id, params }
        }
        AudioCommandPayload::MomentaryFxStop { id } => QueuedAudioEvent::MomentaryFxStop { id },
        AudioCommandPayload::SamplePreview { instrument_slot, sample_slot, path, velocity } => {
            let _sample_slot = sample_slot.min(SAMPLE_SLOTS_PER_INSTRUMENT - 1);
            let full_path = resolve_sample_file(&path).ok_or_else(|| "invalid sample path".to_string())?;
            let buffer = {
                let mut cache = state.sample_cache.lock().map_err(|_| "sample cache poisoned".to_string())?;
                if let Some(buffer) = cache.get(&full_path).cloned() {
                    buffer
                } else {
                    let buffer = decode_sample_file(&full_path).ok_or_else(|| "sample decode failed".to_string())?;
                    cache.insert(full_path, buffer.clone());
                    buffer
                }
            };
            QueuedAudioEvent::PreviewSample { instrument_slot: instrument_slot.min(INSTRUMENT_SLOT_COUNT - 1) as u8, buffer, velocity }
        }
    };
    state
        .trigger_tx
        .send(event)
        .map_err(|e| format!("audio queue send failed: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (trigger_tx, trigger_rx) = mpsc::channel::<QueuedAudioEvent>();
    let (load_tx, load_rx) = mpsc::channel::<realtime_engine::synth::AudioLoadStatus>();

    thread::spawn(move || {
        let (engine_tx, engine_rx) = mpsc::channel::<EngineEvent>();
        let audio = match AudioRuntime::new() {
            Ok(audio) => audio,
            Err(error) => {
                eprintln!("{error}");
                return;
            }
        };

        if let Err(error) = audio.start_engine(engine_rx, load_tx) {
            eprintln!("audio engine start failed: {error}");
            return;
        }

        while let Ok(event) = trigger_rx.recv() {
            match event {
                QueuedAudioEvent::Note(note) => {
                    let _ = engine_tx.send(EngineEvent::NoteOn {
                        instrument_slot: note.instrument_slot,
                        note: note.note,
                        velocity: note.velocity,
                        duration_ms: note.duration_ms,
                    });
                }
                QueuedAudioEvent::Cc {
                    instrument_slot,
                    controller,
                    value,
                } => {
                    let _ = engine_tx.send(EngineEvent::Cc {
                        instrument_slot,
                        controller,
                        value,
                    });
                }
                QueuedAudioEvent::NoteOff {
                    instrument_slot,
                    note,
                } => {
                    let _ = engine_tx.send(EngineEvent::NoteOff {
                        instrument_slot,
                        note,
                    });
                }
                QueuedAudioEvent::PreviewSample { instrument_slot, buffer, velocity } => {
                    let _ = engine_tx.send(EngineEvent::PreviewSample { instrument_slot, buffer, velocity });
                }
                QueuedAudioEvent::SetInstruments(config) => {
                    let _ = engine_tx.send(EngineEvent::SetInstruments(config));
                }
                QueuedAudioEvent::SetSampleBanks(banks) => {
                    let _ = engine_tx.send(EngineEvent::SetSampleBanks(banks));
                }
                QueuedAudioEvent::SetVoiceStealingMode(mode) => {
                    let _ = engine_tx.send(EngineEvent::SetVoiceStealingMode(mode));
                }
                QueuedAudioEvent::MomentaryFxStart {
                    id,
                    fx_type,
                    params,
                    target,
                } => {
                    let _ = engine_tx.send(EngineEvent::MomentaryFxStart {
                        id,
                        fx_type,
                        params,
                        target: match target {
                            MomentaryFxTargetPayload::Global => {
                                realtime_engine::synth::MomentaryFxTarget::Global
                            }
                            MomentaryFxTargetPayload::FxBus { index } => {
                                realtime_engine::synth::MomentaryFxTarget::FxBus { index }
                            }
                            MomentaryFxTargetPayload::Instrument { index } => {
                                realtime_engine::synth::MomentaryFxTarget::Instrument { index }
                            }
                        },
                    });
                }
                QueuedAudioEvent::MomentaryFxUpdate { id, params } => {
                    let _ = engine_tx.send(EngineEvent::MomentaryFxUpdate { id, params });
                }
                QueuedAudioEvent::MomentaryFxStop { id } => {
                    let _ = engine_tx.send(EngineEvent::MomentaryFxStop { id });
                }
            }
        }
    });

    tauri::Builder::default()
        .setup(move |app| {
            let app_handle = app.handle().clone();
            thread::spawn(move || {
                while let Ok(status) = load_rx.recv() {
                    let _ = app_handle.emit(
                        "audio_load",
                        AudioLoadPayload {
                            ratio: status.ratio,
                            voice_steal: status.voice_steal,
                        },
                    );
                }
            });
            Ok(())
        })
        .manage(AppState {
            trigger_tx,
            synth_slots: Mutex::new([true; INSTRUMENT_SLOT_COUNT]),
            sample_cache: Mutex::new(HashMap::new()),
            sample_bank_signature: Mutex::new(String::new()),
            midi_out: Mutex::new(None),
            midi_in: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            trigger_musical_event,
            audio_set_instruments,
            midi_list_outputs,
            midi_list_inputs,
            midi_select_output,
            midi_select_input,
            midi_send,
            sample_list,
            audio_set_runtime_policy,
            audio_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
