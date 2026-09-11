use crate::audio_recording::{self, RecordingServices};
use crate::audio_replay::ReplayCache;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::audio_route::readiness as route_readiness;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use crate::audio_route::status as route_status;
use crate::audio_route::AudioRouteRegistry;
#[cfg(test)]
use crate::audio_sink_registry::test_sink_sender;
use crate::audio_sink_registry::{broadcast_event_atomic, AudioAttachGate, SinkSender};
pub(crate) use crate::audio_stream_health::AudioStreamHealth;
#[cfg(any(
    feature = "hardware-orange-pi-zero-2w",
    all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
        not(feature = "legacy-hardware-rpi-zero-2w"),
        not(feature = "legacy-hardware-pi")
    )
))]
pub(crate) use crate::audio_stream_health::AudioStreamStatus;
use media_recording::{OledFrame, OledIngress, RecordingOutcome, RecordingTap};
#[path = "audio_defaults.rs"]
mod audio_defaults;
#[path = "audio_error.rs"]
mod audio_error;
#[path = "audio_output.rs"]
mod audio_output;
pub(crate) use audio_defaults::default_pi_instruments;
use audio_error::audio_queue_error;
#[cfg(feature = "hardware-raspberry-pi-zero-2w")]
pub(crate) use audio_output::drain_audio_load_status;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use audio_output::OrangeAudioProfile;
pub(crate) use audio_output::{AudioManager, AudioSink};
#[cfg(any(
    feature = "hardware-orange-pi-zero-2w",
    all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
        not(feature = "legacy-hardware-rpi-zero-2w"),
        not(feature = "legacy-hardware-pi")
    )
))]
pub(crate) use audio_output::{
    AudioStreamBuildError, AudioStreamLifecycle, AudioStreamShutdownError,
    AudioStreamShutdownReport, CallbackSource,
};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use playback_runtime::AudioOptimization;
use playback_runtime::AudioOutputSet;
use playback_runtime::{HostMessage, RuntimeAdapterError, RuntimeStoreResult};
use rodio_engine_source::EngineEvent;
#[cfg(test)]
use rodio_engine_source::{event_queue, EngineEventReceiver};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::RwLock;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AudioService {
    realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
    replay_events: Arc<Mutex<ReplayCache>>,
    attach_gate: AudioAttachGate,
    pub control_tx: Sender<AudioControlRequest>,
    pub config_revision: Arc<AtomicU64>,
    pub sample_cache:
        Arc<Mutex<std::collections::HashMap<String, realtime_engine::synth::SampleBuffer>>>,
    pub sample_bank_signature: Arc<Mutex<String>>,
    route_registry: AudioRouteRegistry,
    audio_outputs: AudioOutputSet,
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    required_jack_health: Option<AudioStreamHealth>,
    prep_result_rx: Arc<Mutex<Receiver<HostMessage>>>,
    recorder: Arc<Mutex<RecordingServices>>,
    recording_tap: Arc<RwLock<Option<RecordingTap>>>,
    recording_oled: Arc<RwLock<Option<OledIngress>>>,
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

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) fn orange_profile(optimization: AudioOptimization) -> OrangeAudioProfile {
    OrangeAudioProfile::from_optimization(optimization)
}

impl AudioService {
    pub fn send(&self, event: EngineEvent) -> Result<(), RuntimeAdapterError> {
        self.control_tx
            .send(AudioControlRequest::Dynamic(Box::new(event)))
            .map_err(|error| audio_queue_error(format!("audio control send failed: {error}")))
    }

    pub fn send_realtime(&self, event: EngineEvent) -> Result<(), RuntimeAdapterError> {
        broadcast_event_atomic(
            &self.attach_gate,
            &self.realtime_txs,
            &self.replay_events,
            event,
        )
        .map_err(audio_queue_error)
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

    pub(crate) fn ensure_route_readiness(&self) -> Result<(), String> {
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        if self.required_jack_health.as_ref().is_some_and(|health| {
            health.runtime_status() != crate::audio_stream_health::AudioStreamStatus::Healthy
        }) {
            return Err("required Jack audio stream faulted".into());
        }
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        if route_status(&self.route_registry, AudioSink::Jack)
            == crate::audio_route::AudioRouteStatus::Faulted
        {
            return Err("selected Jack audio route faulted".into());
        }
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let result = Ok(());
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let result = route_readiness(self.audio_outputs, &self.route_registry);
        result
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn usb_output_enabled(&self) -> bool {
        self.audio_outputs.usb()
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn required_jack_failed(&self) -> bool {
        self.required_jack_health.as_ref().is_some_and(|health| {
            health.runtime_status() == crate::audio_stream_health::AudioStreamStatus::Terminal
        })
    }

    pub fn start_recording(&self, max_minutes: u16) -> Result<(), String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        let tap = recorder
            .start_audio(max_minutes)
            .map_err(|error| error.to_string())?;
        *self
            .recording_oled
            .write()
            .map_err(|_| "OLED recording lock poisoned".to_string())? = None;
        *self
            .recording_tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = Some(tap);
        Ok(())
    }

    pub(crate) fn start_recording_audio_oled_with_seed(
        &self,
        max_minutes: u16,
        seed: Option<(u64, Vec<u8>)>,
    ) -> Result<(), String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        let recording = recorder
            .start_audio_oled(max_minutes)
            .map_err(|error| error.to_string())?;
        if let Some((revision, pixels)) = seed {
            let frame =
                OledFrame::from_bytes(revision, 0, pixels).map_err(|error| error.to_string())?;
            let _ = recording.oled.try_submit(frame);
        }
        *self
            .recording_tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = Some(recording.tap);
        *self
            .recording_oled
            .write()
            .map_err(|_| "OLED recording lock poisoned".to_string())? = Some(recording.oled);
        Ok(())
    }

    pub fn stop_recording(&self) -> Result<(), String> {
        self.stop_recording_with_outcome().map(|_| ())
    }

    pub(crate) fn stop_recording_with_outcome(&self) -> Result<Option<RecordingOutcome>, String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        *self
            .recording_tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = None;
        *self
            .recording_oled
            .write()
            .map_err(|_| "OLED recording lock poisoned".to_string())? = None;
        let outcome = recorder.stop_audio().map_err(|error| error.to_string())?;
        if let Some(outcome) = &outcome {
            println!(
                "recording stopped: path={} frames={} status={:?}",
                outcome.path.display(),
                outcome.frames_written,
                outcome.status
            );
        }
        Ok(outcome)
    }

    pub(crate) fn poll_recording_status(&self) -> Option<RuntimeStoreResult> {
        audio_recording::poll_recording_status(
            &self.recorder,
            &self.recording_tap,
            &self.recording_oled,
        )
    }

    pub(crate) fn prepare_restore(&self) -> Result<(), String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        let active = recorder.is_recording();
        if active {
            recorder.stop_audio().map_err(|error| error.to_string())?;
        }
        *self
            .recording_tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = None;
        *self
            .recording_oled
            .write()
            .map_err(|_| "OLED recording lock poisoned".to_string())? = None;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn is_recording(&self) -> Result<bool, String> {
        self.recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())
            .map(|recorder| recorder.is_recording())
    }

    pub(crate) fn submit_accepted_oled_frame(
        &self,
        revision: u64,
        pixels: &[u8],
    ) -> Result<(), String> {
        if !self.is_recording()? {
            return Ok(());
        }
        let tap = self
            .recording_tap
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .clone();
        let oled = self
            .recording_oled
            .read()
            .map_err(|_| "OLED recording lock poisoned".to_string())?
            .clone();
        let Some((tap, oled)) = tap.zip(oled) else {
            return Ok(());
        };
        let frame = OledFrame::from_bytes(revision, tap.audio_frame_cursor(), pixels)
            .map_err(|error| error.to_string())?;
        let _ = oled.try_submit(frame);
        Ok(())
    }

    #[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn test_push_recording_samples(&self, samples: &[i16]) -> Result<(), String> {
        let tap = self
            .recording_tap
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .clone()
            .ok_or_else(|| "recording tap is inactive".to_string())?;
        let mut chunk = tap.new_chunk();
        let (frames, _) = samples.as_chunks::<2>();
        for frame in frames {
            if !chunk.push_frame(frame[0], frame[1]) {
                tap.push_chunk(chunk);
                chunk = tap.new_chunk();
                assert!(chunk.push_frame(frame[0], frame[1]));
            }
        }
        if !chunk.is_empty() {
            tap.push_chunk(chunk);
        }
        Ok(())
    }
}

impl AudioService {
    pub(crate) fn broadcast(&self, event: EngineEvent) -> Result<(), String> {
        broadcast_event_atomic(
            &self.attach_gate,
            &self.realtime_txs,
            &self.replay_events,
            event,
        )
    }
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use tests::test_service_with_prep_result_sender;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use tests::test_service_with_prep_worker;
#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) use tests::{test_service, test_service_with_outputs, test_service_with_prep_sender};
#[cfg(test)]
pub(crate) use tests::{test_service_for_sample_prep, test_service_with_recording_dir};

#[cfg(test)]
#[path = "audio_oled_recording_tests.rs"]
mod audio_oled_recording_tests;
#[cfg(test)]
#[path = "audio_service_tests.rs"]
mod tests;
