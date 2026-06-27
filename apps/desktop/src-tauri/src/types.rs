use playback_runtime::RunnerMessage;
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use rodio_engine_source::{EngineEvent, EngineSource};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
#[serde(tag = "type")]
pub(crate) enum AudioCommandPayload {
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

#[derive(Clone, Default, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum MomentaryFxTargetPayload {
    #[default]
    #[serde(rename = "global")]
    Global,
    #[serde(rename = "fx_bus")]
    FxBus { index: usize },
    #[serde(rename = "instrument")]
    Instrument { index: usize },
}

pub(crate) struct AudioRuntime {
    _stream: OutputStream,
    handle: OutputStreamHandle,
}

impl AudioRuntime {
    pub(crate) fn new() -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| format!("audio init failed: {e}"))?;
        Ok(Self {
            _stream: stream,
            handle,
        })
    }

    pub(crate) fn start_engine(
        &self,
        control_rx: Receiver<EngineEvent>,
        load_tx: Sender<realtime_engine::synth::AudioLoadStatus>,
    ) -> Result<(), String> {
        let source =
            EngineSource::with_load_status_tx(control_rx, DEFAULT_AUDIO_SAMPLE_RATE, Some(load_tx));
        let sink = Sink::try_new(&self.handle).map_err(|e| format!("sink create failed: {e}"))?;
        sink.append(source);
        sink.play();
        sink.detach();
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct QueuedNote {
    pub(crate) instrument_slot: u8,
    pub(crate) note: u8,
    pub(crate) velocity: u8,
    pub(crate) duration_ms: u32,
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
    SetAudioConfig {
        instruments: realtime_engine::synth::InstrumentsConfig,
        sample_banks: Option<Vec<realtime_engine::synth::SampleBankConfig>>,
        voice_stealing_mode: Option<realtime_engine::synth::VoiceStealingMode>,
    },
    SetMasterVolume {
        volume_pct: f32,
    },
    SetInstrumentMixer {
        instrument_slot: usize,
        volume_pct: Option<f32>,
        pan_pos: Option<usize>,
    },
    SetFxBusMixer {
        bus_index: usize,
        pan_pos: Option<usize>,
    },
    SetSynthParam {
        instrument_slot: usize,
        path: String,
        value: f32,
    },
    SetSampleBankParam {
        instrument_slot: usize,
        path: String,
        value: f32,
    },
    SetFxBusSlot {
        bus_index: usize,
        slot_index: usize,
        fx_type: String,
        params: BTreeMap<String, Value>,
    },
    SetGlobalFxSlot {
        slot_index: usize,
        fx_type: String,
        params: BTreeMap<String, Value>,
    },
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

pub(crate) const RUNTIME_MESSAGES_EVENT: &str = "runtime_messages";
pub(crate) const RUNTIME_UI_REFRESH_MS: u64 = 100;

#[derive(Clone, serde::Serialize)]
pub(crate) struct RuntimeMessagesPayload {
    pub(crate) seq: u64,
    pub(crate) messages: Vec<Value>,
}

pub(crate) fn encode_runtime_responses(
    responses: Vec<RunnerMessage>,
) -> Result<Vec<Value>, String> {
    responses
        .into_iter()
        .map(|r| {
            serde_json::to_value(r).map_err(|e| format!("failed to encode runtime response: {e}"))
        })
        .collect()
}

pub(crate) fn append_audio_error_values(
    mut values: Vec<Value>,
    audio_error: &Arc<Mutex<Option<String>>>,
) -> Vec<Value> {
    if let Ok(guard) = audio_error.lock() {
        if let Some(err) = guard.as_ref() {
            values.push(serde_json::json!({ "type": "audio_error", "error": err }));
        }
    }
    values
}
