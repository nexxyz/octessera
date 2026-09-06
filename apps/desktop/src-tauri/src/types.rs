use playback_runtime::RunnerMessage;
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use rodio_engine_source::{AudioLoadStatusSender, EngineEventReceiver, EngineSource};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

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
    sink: Option<Sink>,
}

impl AudioRuntime {
    pub(crate) fn new() -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| format!("audio init failed: {e}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            sink: None,
        })
    }

    pub(crate) fn start_engine(
        &mut self,
        control_rx: EngineEventReceiver,
        load_tx: AudioLoadStatusSender,
    ) -> Result<(), String> {
        self.stop();
        let source =
            EngineSource::with_load_status_tx(control_rx, DEFAULT_AUDIO_SAMPLE_RATE, Some(load_tx));
        let sink = match Sink::try_new(&self.handle) {
            Ok(sink) => sink,
            Err(error) => {
                drop(source);
                self.stop();
                return Err(format!("sink create failed: {error}"));
            }
        };
        sink.append(source);
        sink.play();
        self.sink = Some(sink);
        Ok(())
    }

    pub(crate) fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
            drop(sink);
        }
    }
}

impl Drop for AudioRuntime {
    fn drop(&mut self) {
        self.stop();
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
    AllNotesOff,
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
        revision: u64,
        request_id: Option<String>,
        instruments: realtime_engine::synth::InstrumentsConfig,
        sample_banks: Option<Vec<realtime_engine::synth::SampleBankConfig>>,
        voice_stealing_mode: Option<realtime_engine::synth::VoiceStealingMode>,
    },
    SetMasterVolume {
        volume_pct: f32,
    },
    SetDspConfig {
        config: realtime_engine::synth::DspRuntimeConfig,
    },
    SetInstrumentMixer {
        instrument_slot: usize,
        volume_pct: Option<f32>,
        pan_pos: Option<usize>,
    },
    SetInstrumentSlot {
        instrument_slot: usize,
        config: realtime_engine::synth::InstrumentSlotConfig,
        sample_bank: Option<realtime_engine::synth::SampleBankConfig>,
    },
    SetFxBusMixer {
        bus_index: usize,
        pan_pos: Option<usize>,
        volume_pct: Option<f32>,
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
