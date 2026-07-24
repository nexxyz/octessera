use crate::audio_hotplug::{broadcast_event, is_replay_event, ReplayCache, SinkSender};
use crate::recording::{RecorderService, RecordingTap};
#[path = "audio_defaults.rs"]
mod audio_defaults;
#[path = "audio_error.rs"]
mod audio_error;
#[path = "audio_output.rs"]
mod audio_output;
pub(crate) use audio_defaults::default_pi_instruments;
use audio_error::audio_queue_error;
#[cfg(test)]
pub(crate) use audio_output::audio_sinks;
pub(crate) use audio_output::{AudioManager, AudioSink};
use playback_runtime::{HostMessage, RuntimeAdapterError};
use rodio_engine_source::EngineEvent;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, RwLock};

#[derive(Clone)]
pub struct AudioService {
    realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
    replay_events: Arc<Mutex<ReplayCache>>,
    pub control_tx: Sender<AudioControlRequest>,
    pub config_revision: Arc<AtomicU64>,
    pub sample_cache:
        Arc<Mutex<std::collections::HashMap<String, realtime_engine::synth::SampleBuffer>>>,
    pub sample_bank_signature: Arc<Mutex<String>>,
    prep_result_rx: Arc<Mutex<Receiver<HostMessage>>>,
    recorder: Arc<Mutex<RecorderService>>,
    recording_tap: Arc<RwLock<Option<RecordingTap>>>,
}

pub enum AudioControlRequest {
    FullConfig {
        revision: u64,
        request_id: Option<String>,
        config: Value,
        samples_dir: PathBuf,
    },
    SamplePreview {
        instrument_slot: usize,
        path: String,
        velocity: u8,
        samples_dir: PathBuf,
    },
    Dynamic(Box<EngineEvent>),
}

impl AudioService {
    pub fn send(&self, event: EngineEvent) -> Result<(), RuntimeAdapterError> {
        self.control_tx
            .send(AudioControlRequest::Dynamic(Box::new(event)))
            .map_err(|error| audio_queue_error(format!("audio control send failed: {error}")))
    }

    pub fn send_realtime(&self, event: EngineEvent) -> Result<(), RuntimeAdapterError> {
        self.remember_replay_event(&event);
        broadcast_event(&self.realtime_txs, event).map_err(audio_queue_error)
    }

    pub fn enqueue_full_config(
        &self,
        revision: u64,
        request_id: Option<String>,
        config: Value,
        samples_dir: PathBuf,
    ) -> Result<(), String> {
        self.control_tx
            .send(AudioControlRequest::FullConfig {
                revision,
                request_id,
                config,
                samples_dir,
            })
            .map_err(|e| format!("audio prep send failed: {e}"))
    }

    pub fn enqueue_sample_preview(
        &self,
        instrument_slot: usize,
        path: String,
        velocity: u8,
        samples_dir: PathBuf,
    ) -> Result<(), String> {
        self.control_tx
            .send(AudioControlRequest::SamplePreview {
                instrument_slot,
                path,
                velocity,
                samples_dir,
            })
            .map_err(|e| format!("sample preview prep send failed: {e}"))
    }

    pub fn drain_prep_results(&self, max_results: usize) -> Vec<HostMessage> {
        let Ok(results) = self.prep_result_rx.lock() else {
            return Vec::new();
        };
        let mut output = Vec::new();
        for _ in 0..max_results {
            match results.try_recv() {
                Ok(result) => output.push(result),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        output
    }

    pub fn start_recording(&self, max_minutes: u16) -> Result<(), String> {
        let tap = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?
            .start_audio(max_minutes)?;
        *self
            .recording_tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = Some(tap);
        Ok(())
    }

    pub fn stop_recording(&self) -> Result<(), String> {
        *self
            .recording_tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = None;
        self.recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?
            .stop_audio();
        Ok(())
    }

    pub fn is_recording(&self) -> Result<bool, String> {
        Ok(self
            .recording_tap
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .is_some())
    }
}

impl AudioService {
    pub(crate) fn remember_replay_event(&self, event: &EngineEvent) {
        if !is_replay_event(event) {
            return;
        }
        if let Ok(mut events) = self.replay_events.lock() {
            events.remember(event);
        }
    }

    pub(crate) fn broadcast(&self, event: EngineEvent) -> Result<(), String> {
        self.remember_replay_event(&event);
        broadcast_event(&self.realtime_txs, event)
    }
}
