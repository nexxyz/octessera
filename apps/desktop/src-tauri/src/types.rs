use crate::recording::{RecordingEngineSource, RecordingTapState};
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
        #[serde(default)]
        epoch: u64,
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
        epoch: u64,
        #[serde(default)]
        params: BTreeMap<String, Value>,
    },
    #[serde(rename = "momentary_fx_stop")]
    MomentaryFxStop {
        id: String,
        #[serde(default)]
        epoch: u64,
    },
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
    recording_tap: RecordingTapState,
}

impl AudioRuntime {
    pub(crate) fn new(recording_tap: RecordingTapState) -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| format!("audio init failed: {e}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            sink: None,
            recording_tap,
        })
    }

    pub(crate) fn start_engine(
        &mut self,
        control_rx: EngineEventReceiver,
        load_tx: AudioLoadStatusSender,
    ) -> Result<(), String> {
        self.stop();
        let source = RecordingEngineSource::new(
            EngineSource::with_load_status_tx(control_rx, DEFAULT_AUDIO_SAMPLE_RATE, Some(load_tx)),
            self.recording_tap.clone(),
        );
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
