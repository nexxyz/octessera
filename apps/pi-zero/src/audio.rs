use crate::audio_replay::ReplayCache;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::audio_route::readiness as route_readiness;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use crate::audio_route::status as route_status;
use crate::audio_route::AudioRouteRegistry;
#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
use crate::audio_sink_registry::test_sink_sender;
use crate::audio_sink_registry::{broadcast_event_atomic, AudioAttachGate, SinkSender};
pub(crate) use crate::audio_stream_health::AudioStreamHealth;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) use crate::audio_stream_health::AudioStreamStatus;
use crate::recording::{RecorderService, RecordingTap};
#[path = "audio_defaults.rs"]
mod audio_defaults;
#[path = "audio_error.rs"]
mod audio_error;
#[path = "audio_output.rs"]
mod audio_output;
pub(crate) use audio_defaults::default_pi_instruments;
use audio_error::audio_queue_error;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use audio_output::OrangeAudioProfile;
pub(crate) use audio_output::{AudioManager, AudioSink};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) use audio_output::{
    AudioStreamBuildError, AudioStreamLifecycle, AudioStreamShutdownError,
    AudioStreamShutdownReport, CallbackSource,
};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use playback_runtime::AudioOptimization;
use playback_runtime::AudioOutputSet;
use playback_runtime::{HostMessage, RuntimeAdapterError};
use rodio_engine_source::EngineEvent;
#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
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
        if self
            .required_jack_health
            .as_ref()
            .is_some_and(AudioStreamHealth::external_is_faulted)
        {
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
        self.required_jack_health
            .as_ref()
            .is_some_and(AudioStreamHealth::external_is_faulted)
    }

    pub fn start_recording(&self, max_minutes: u16) -> Result<(), String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        let tap = recorder.start_audio(max_minutes)?;
        *self
            .recording_tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = Some(tap);
        Ok(())
    }

    pub fn stop_recording(&self) -> Result<(), String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        *self
            .recording_tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = None;
        recorder.stop_audio();
        Ok(())
    }

    pub(crate) fn prepare_restore(&self) -> Result<(), String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        let active = self
            .recording_tap
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .is_some();
        if active {
            *self
                .recording_tap
                .write()
                .map_err(|_| "recording tap lock poisoned".to_string())? = None;
            recorder.stop_audio();
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn is_recording(&self) -> Result<bool, String> {
        Ok(self
            .recording_tap
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .is_some())
    }

    #[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn test_push_recording_samples(&self, samples: &[i16]) -> Result<(), String> {
        let tap = self
            .recording_tap
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .clone()
            .ok_or_else(|| "recording tap is inactive".to_string())?;
        let mut chunk = crate::recording::RecordingChunk::new();
        for sample in samples {
            if !chunk.push(*sample) {
                tap.push_chunk(chunk.take());
                assert!(chunk.push(*sample));
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

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn test_service() -> (
    AudioService,
    Receiver<AudioControlRequest>,
    EngineEventReceiver,
) {
    let (service, control_rx, event_rx, _) = test_service_with_prep_sender();
    (service, control_rx, event_rx)
}

#[cfg(test)]
pub(crate) fn test_service_for_sample_prep() -> AudioService {
    test_service_with_prep_result_sender().0
}

#[cfg(test)]
pub(crate) fn test_service_with_prep_result_sender() -> (AudioService, Sender<HostMessage>) {
    let (control_tx, _control_rx) = std::sync::mpsc::channel();
    let (prep_result_tx, prep_result_rx) = std::sync::mpsc::channel();
    let service = AudioService {
        realtime_txs: Arc::new(Mutex::new(Vec::new())),
        replay_events: Arc::new(Mutex::new(ReplayCache::default())),
        attach_gate: crate::audio_sink_registry::new_attach_gate(),
        control_tx,
        config_revision: Arc::new(AtomicU64::new(0)),
        sample_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        sample_bank_signature: Arc::new(Mutex::new(String::new())),
        route_registry: crate::audio_route::new_registry(AudioOutputSet::jack()),
        audio_outputs: AudioOutputSet::jack(),
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        required_jack_health: None,
        prep_result_rx: Arc::new(Mutex::new(prep_result_rx)),
        recorder: Arc::new(Mutex::new(crate::recording::RecorderService::new(
            std::env::temp_dir().join("octessera-sample-prep-recordings"),
        ))),
        recording_tap: Arc::new(RwLock::new(None)),
    };
    (service, prep_result_tx)
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) fn test_service_with_prep_worker() -> AudioService {
    let (control_tx, control_rx) = std::sync::mpsc::channel();
    let (prep_result_tx, prep_result_rx) = std::sync::mpsc::channel();
    let service = AudioService {
        realtime_txs: Arc::new(Mutex::new(Vec::new())),
        replay_events: Arc::new(Mutex::new(ReplayCache::default())),
        attach_gate: crate::audio_sink_registry::new_attach_gate(),
        control_tx,
        config_revision: Arc::new(AtomicU64::new(0)),
        sample_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        sample_bank_signature: Arc::new(Mutex::new(String::new())),
        route_registry: crate::audio_route::new_registry(AudioOutputSet::jack()),
        audio_outputs: AudioOutputSet::jack(),
        required_jack_health: None,
        prep_result_rx: Arc::new(Mutex::new(prep_result_rx)),
        recorder: Arc::new(Mutex::new(crate::recording::RecorderService::new(
            std::env::temp_dir().join("octessera-sample-prep-recordings"),
        ))),
        recording_tap: Arc::new(RwLock::new(None)),
    };
    crate::host_audio_prep::spawn_audio_control_worker(control_rx, service.clone(), prep_result_tx);
    service
}

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn test_service_with_prep_sender() -> (
    AudioService,
    Receiver<AudioControlRequest>,
    EngineEventReceiver,
    Sender<HostMessage>,
) {
    test_service_with_recording_dir(
        std::env::temp_dir().join("octessera-orange-sample-prep-recordings"),
    )
}

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn test_service_with_outputs(outputs: AudioOutputSet) -> AudioService {
    let (mut service, _, _, _) = test_service_with_recording_dir(
        std::env::temp_dir().join("octessera-orange-gate-recordings"),
    );
    service.audio_outputs = outputs;
    service
}

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn test_service_with_recording_dir(
    recording_dir: std::path::PathBuf,
) -> (
    AudioService,
    Receiver<AudioControlRequest>,
    EngineEventReceiver,
    Sender<HostMessage>,
) {
    let (event_tx, event_rx) = event_queue();
    let (control_tx, control_rx) = std::sync::mpsc::channel();
    let (prep_result_tx, prep_result_rx) = std::sync::mpsc::channel();
    let service = AudioService {
        realtime_txs: Arc::new(Mutex::new(vec![test_sink_sender(event_tx)])),
        replay_events: Arc::new(Mutex::new(ReplayCache::default())),
        attach_gate: crate::audio_sink_registry::new_attach_gate(),
        control_tx,
        config_revision: Arc::new(AtomicU64::new(0)),
        sample_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        sample_bank_signature: Arc::new(Mutex::new(String::new())),
        route_registry: crate::audio_route::new_registry(AudioOutputSet::jack()),
        audio_outputs: AudioOutputSet::jack(),
        prep_result_rx: Arc::new(Mutex::new(prep_result_rx)),
        recorder: Arc::new(Mutex::new(crate::recording::RecorderService::new(
            recording_dir,
        ))),
        recording_tap: Arc::new(RwLock::new(None)),
    };
    (service, control_rx, event_rx, prep_result_tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_preflight_finalizes_active_recording() {
        let service = test_service_for_sample_prep();
        service.start_recording(1).unwrap();
        assert!(service.is_recording().unwrap());
        service.prepare_restore().unwrap();
        assert!(!service.is_recording().unwrap());
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    #[test]
    fn raspberry_optional_route_fault_does_not_block_jack_readiness() {
        let mut service = test_service_for_sample_prep();
        service.audio_outputs = AudioOutputSet::from_flags(true, true, false).unwrap();
        crate::audio_route::set_status(
            &service.route_registry,
            AudioSink::Jack,
            crate::audio_route::AudioRouteStatus::Active,
        );
        crate::audio_route::set_status(
            &service.route_registry,
            AudioSink::Usb,
            crate::audio_route::AudioRouteStatus::Faulted,
        );

        assert!(service.ensure_route_readiness().is_ok());
    }
}
