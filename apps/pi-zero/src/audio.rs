use crate::audio_recording::RecordingServices;
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
use media_recording::{OledIngress, RecordingTap};
#[path = "audio_defaults.rs"]
mod audio_defaults;
#[path = "audio_error.rs"]
mod audio_error;
#[path = "audio_output.rs"]
mod audio_output;
#[path = "audio_recording_service.rs"]
mod audio_recording_service;
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
use playback_runtime::{HostMessage, RuntimeAdapterError};
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use rodio_engine_source::EngineEvent;
#[cfg(test)]
use rodio_engine_source::{event_queue, EngineEventReceiver};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::RwLock;
use std::sync::{Arc, Mutex};

pub(crate) const AUDIO_PREP_QUEUE_CAPACITY: usize = 32;

#[derive(Clone)]
pub(crate) struct AudioGenerationState {
    pub(crate) full: u64,
    pub(crate) instrument: [u64; INSTRUMENT_SLOT_COUNT],
    pub(crate) sample: [u64; INSTRUMENT_SLOT_COUNT],
}

impl Default for AudioGenerationState {
    fn default() -> Self {
        Self {
            full: 0,
            instrument: [0; INSTRUMENT_SLOT_COUNT],
            sample: [0; INSTRUMENT_SLOT_COUNT],
        }
    }
}

#[derive(Clone)]
pub struct AudioService {
    realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
    replay_events: Arc<Mutex<ReplayCache>>,
    attach_gate: AudioAttachGate,
    pub control_tx: SyncSender<AudioControlRequest>,
    pub config_revision: Arc<AtomicU64>,
    pub sample_cache:
        Arc<Mutex<std::collections::HashMap<String, realtime_engine::synth::SampleBuffer>>>,
    pub sample_bank_signature: Arc<Mutex<String>>,
    pub preview_generation: Arc<AtomicU64>,
    pub momentary_fx_types: Arc<Mutex<std::collections::BTreeMap<String, (u64, String)>>>,
    pub(crate) next_sequence: Arc<AtomicU64>,
    pub(crate) latest_full_sequence: Arc<AtomicU64>,
    pub(crate) generations: Arc<Mutex<AudioGenerationState>>,
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
        sequence: u64,
        revision: u64,
        generation: u64,
        request_id: Option<String>,
        config: Value,
        samples_dir: PathBuf,
    },
    InstrumentSlot {
        sequence: u64,
        instrument_slot: usize,
        generation: u64,
        config: Value,
        samples_dir: PathBuf,
    },
    FxBusSlot {
        sequence: u64,
        bus_index: usize,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: std::collections::BTreeMap<String, Value>,
    },
    GlobalFxSlot {
        sequence: u64,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: std::collections::BTreeMap<String, Value>,
    },
    SamplePreview {
        sequence: u64,
        instrument_slot: usize,
        path: String,
        velocity: u8,
        samples_dir: PathBuf,
        preview_token: u64,
    },
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) fn orange_profile(optimization: AudioOptimization) -> OrangeAudioProfile {
    OrangeAudioProfile::from_optimization(optimization)
}

impl AudioService {
    pub fn send(&self, event: EngineEvent) -> Result<(), RuntimeAdapterError> {
        broadcast_event_atomic(
            &self.attach_gate,
            &self.realtime_txs,
            &self.replay_events,
            event,
        )
        .map_err(audio_queue_error)
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

    pub fn remember_momentary_fx_type(
        &self,
        id: &str,
        epoch: u64,
        fx_type: &str,
    ) -> Result<(), String> {
        self.momentary_fx_types
            .lock()
            .map_err(|_| "momentary FX type lock failed".to_string())?
            .insert(id.to_string(), (epoch, fx_type.to_string()));
        Ok(())
    }

    pub fn momentary_fx_type(&self, id: &str) -> Result<Option<(u64, String)>, String> {
        self.momentary_fx_types
            .lock()
            .map_err(|_| "momentary FX type lock failed".to_string())
            .map(|types| types.get(id).cloned())
    }

    pub fn remove_momentary_fx_type(&self, id: &str, epoch: u64) -> Result<(), String> {
        let mut types = self
            .momentary_fx_types
            .lock()
            .map_err(|_| "momentary FX type lock failed".to_string())?;
        if types
            .get(id)
            .is_some_and(|(active_epoch, _)| *active_epoch == epoch)
        {
            types.remove(id);
        }
        Ok(())
    }

    pub(crate) fn next_sequence(&self) -> u64 {
        self.next_sequence.fetch_add(1, Ordering::Relaxed)
    }

    pub fn enqueue_full_config(
        &self,
        revision: u64,
        generation: u64,
        request_id: Option<String>,
        config: Value,
        samples_dir: PathBuf,
    ) -> Result<(), String> {
        let previous = self.config_revision.fetch_max(revision, Ordering::AcqRel);
        let sequence = self.next_sequence();
        match self.control_tx.try_send(AudioControlRequest::FullConfig {
            sequence,
            revision,
            generation,
            request_id,
            config,
            samples_dir,
        }) {
            Ok(()) => {
                self.latest_full_sequence
                    .fetch_max(sequence, Ordering::Release);
                Ok(())
            }
            Err(error) => {
                let _ = self.config_revision.compare_exchange(
                    revision.max(previous),
                    previous,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                Err(format!("audio prep queue failed: {error}"))
            }
        }
    }

    pub fn enqueue_instrument_slot(
        &self,
        instrument_slot: usize,
        generation: u64,
        config: Value,
        samples_dir: PathBuf,
    ) -> Result<(), String> {
        self.control_tx
            .try_send(AudioControlRequest::InstrumentSlot {
                sequence: self.next_sequence(),
                instrument_slot,
                generation,
                config,
                samples_dir,
            })
            .map_err(|e| format!("audio prep queue failed: {e}"))
    }

    pub fn enqueue_fx_bus_slot(
        &self,
        bus_index: usize,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: std::collections::BTreeMap<String, Value>,
    ) -> Result<(), String> {
        self.control_tx
            .try_send(AudioControlRequest::FxBusSlot {
                sequence: self.next_sequence(),
                bus_index,
                slot_index,
                generation,
                fx_type,
                params,
            })
            .map_err(|e| format!("audio prep queue failed: {e}"))
    }

    pub fn enqueue_global_fx_slot(
        &self,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: std::collections::BTreeMap<String, Value>,
    ) -> Result<(), String> {
        self.control_tx
            .try_send(AudioControlRequest::GlobalFxSlot {
                sequence: self.next_sequence(),
                slot_index,
                generation,
                fx_type,
                params,
            })
            .map_err(|e| format!("audio prep queue failed: {e}"))
    }

    pub fn enqueue_sample_preview(
        &self,
        instrument_slot: usize,
        path: String,
        velocity: u8,
        samples_dir: PathBuf,
    ) -> Result<(), String> {
        let preview_generation = self.preview_generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.control_tx
            .try_send(AudioControlRequest::SamplePreview {
                sequence: self.next_sequence(),
                instrument_slot,
                path,
                velocity,
                samples_dir,
                preview_token: preview_generation,
            })
            .map_err(|e| format!("sample preview queue failed: {e}"))
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

    pub(crate) fn sample_owner_generation(&self, instrument_slot: usize) -> Result<u64, String> {
        self.generations
            .lock()
            .map_err(|_| "audio generation state lock failed".to_string())
            .map(|generations| {
                generations
                    .sample
                    .get(instrument_slot)
                    .copied()
                    .unwrap_or_default()
            })
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
}

impl AudioService {
    pub(crate) fn broadcast(&self, event: EngineEvent) -> Result<(), String> {
        broadcast_event_atomic(
            &self.attach_gate,
            &self.realtime_txs,
            &self.replay_events,
            event,
        )
        .map_err(|error| error.to_string())
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
