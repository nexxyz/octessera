#[path = "audio_prep_config.rs"]
mod audio_prep_config;
#[path = "audio_prep_results.rs"]
mod audio_prep_results;
#[path = "audio_prep_worker.rs"]
mod audio_prep_worker;

use crate::sample_decode_cache::SampleDecodeCache;
use audio_prep_worker::audio_control_loop;
use playback_runtime::HostMessage;
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use rodio_engine_source::EngineEventSender;
use serde_json::Value;
#[cfg(test)]
use std::collections::VecDeque;
#[cfg(test)]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};

pub(crate) const AUDIO_PREP_QUEUE_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioPrepEnqueueResult {
    Accepted,
    Full,
    Disconnected,
}

#[derive(Clone)]
pub(crate) struct AudioGenerationState {
    pub(crate) full: u64,
    pub(crate) instrument: [u64; INSTRUMENT_SLOT_COUNT],
    pub(crate) sample: [u64; INSTRUMENT_SLOT_COUNT],
    pub(crate) bus_mixer: [u64; realtime_engine::synth::BUS_COUNT],
    pub(crate) fx_bus:
        [[u64; realtime_engine::synth::BUS_SLOTS_PER_BUS]; realtime_engine::synth::BUS_COUNT],
    pub(crate) global_fx: [u64; realtime_engine::synth::GLOBAL_FX_SLOT_COUNT],
}

impl Default for AudioGenerationState {
    fn default() -> Self {
        Self {
            full: 0,
            instrument: [0; INSTRUMENT_SLOT_COUNT],
            sample: [0; INSTRUMENT_SLOT_COUNT],
            bus_mixer: [0; realtime_engine::synth::BUS_COUNT],
            fx_bus: [[0; realtime_engine::synth::BUS_SLOTS_PER_BUS];
                realtime_engine::synth::BUS_COUNT],
            global_fx: [0; realtime_engine::synth::GLOBAL_FX_SLOT_COUNT],
        }
    }
}

#[derive(Clone)]
pub(crate) struct DesktopAudioControl {
    tx: SyncSender<AudioControlRequest>,
    config_revision: Arc<AtomicU64>,
    next_sequence: Arc<AtomicU64>,
    next_preview_token: Arc<AtomicU64>,
    preview_latest: Arc<Mutex<Option<PreviewRequest>>>,
    generations: Arc<Mutex<AudioGenerationState>>,
    #[cfg(test)]
    preview_prepare_gate: Option<Arc<PreviewPrepareTestGate>>,
}

#[cfg(test)]
struct PreviewPrepareTestGate {
    started: AtomicBool,
    release: AtomicBool,
}

pub(crate) struct DesktopAudioPrepState {
    pub(crate) config_revision: Arc<AtomicU64>,
    pub(crate) synth_slots: Arc<Mutex<[bool; INSTRUMENT_SLOT_COUNT]>>,
    pub(crate) sample_decode_cache: SampleDecodeCache,
    pub(crate) sample_bank_signature: Arc<Mutex<String>>,
    pub(crate) generations: Arc<Mutex<AudioGenerationState>>,
}

#[derive(Clone)]
enum AudioControlRequest {
    FullConfig {
        sequence: u64,
        revision: u64,
        generation: u64,
        request_id: Option<String>,
        config: Value,
    },
    InstrumentSlot {
        sequence: u64,
        instrument_slot: usize,
        generation: u64,
        config: Value,
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
    SamplePreview(PreviewRequest),
}

#[derive(Clone)]
struct PreviewRequest {
    sequence: u64,
    token: u64,
    instrument_slot: usize,
    path: String,
    velocity: u8,
}

pub(crate) fn spawn_desktop_audio_control(
    engine_tx: EngineEventSender,
    state: DesktopAudioPrepState,
) -> (DesktopAudioControl, Receiver<HostMessage>) {
    let (tx, rx) = mpsc::sync_channel(AUDIO_PREP_QUEUE_CAPACITY);
    let (result_tx, result_rx) = mpsc::channel();
    let control = DesktopAudioControl {
        tx,
        config_revision: state.config_revision.clone(),
        next_sequence: Arc::new(AtomicU64::new(0)),
        next_preview_token: Arc::new(AtomicU64::new(0)),
        preview_latest: Arc::new(Mutex::new(None)),
        generations: state.generations.clone(),
        #[cfg(test)]
        preview_prepare_gate: None,
    };
    let worker_control = control.clone();
    std::thread::spawn(move || audio_control_loop(rx, engine_tx, result_tx, state, worker_control));
    (control, result_rx)
}

impl DesktopAudioControl {
    pub(crate) fn enqueue_full_config(
        &self,
        revision: u64,
        generation: u64,
        request_id: Option<String>,
        config: Value,
    ) -> AudioPrepEnqueueResult {
        let previous_revision = self.config_revision.fetch_max(revision, Ordering::SeqCst);
        let request = AudioControlRequest::FullConfig {
            sequence: self.next_sequence(),
            revision,
            generation,
            request_id,
            config,
        };
        let result = self.try_send(request);
        if result != AudioPrepEnqueueResult::Accepted && revision > previous_revision {
            let _ = self.config_revision.compare_exchange(
                revision,
                previous_revision,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
        result
    }

    pub(crate) fn enqueue_instrument_slot(
        &self,
        instrument_slot: usize,
        generation: u64,
        config: Value,
    ) -> AudioPrepEnqueueResult {
        self.try_send(AudioControlRequest::InstrumentSlot {
            sequence: self.next_sequence(),
            instrument_slot,
            generation,
            config,
        })
    }

    pub(crate) fn enqueue_fx_bus_slot(
        &self,
        bus_index: usize,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: std::collections::BTreeMap<String, Value>,
    ) -> AudioPrepEnqueueResult {
        self.try_send(AudioControlRequest::FxBusSlot {
            sequence: self.next_sequence(),
            bus_index,
            slot_index,
            generation,
            fx_type,
            params,
        })
    }

    pub(crate) fn enqueue_global_fx_slot(
        &self,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: std::collections::BTreeMap<String, Value>,
    ) -> AudioPrepEnqueueResult {
        self.try_send(AudioControlRequest::GlobalFxSlot {
            sequence: self.next_sequence(),
            slot_index,
            generation,
            fx_type,
            params,
        })
    }

    pub(crate) fn enqueue_sample_preview(
        &self,
        instrument_slot: usize,
        path: String,
        velocity: u8,
    ) -> AudioPrepEnqueueResult {
        let token = self.next_preview_token.fetch_add(1, Ordering::SeqCst) + 1;
        let request = PreviewRequest {
            sequence: self.next_sequence(),
            token,
            instrument_slot,
            path,
            velocity,
        };
        match self
            .tx
            .try_send(AudioControlRequest::SamplePreview(request.clone()))
        {
            Ok(()) => AudioPrepEnqueueResult::Accepted,
            Err(mpsc::TrySendError::Full(_)) => {
                if let Ok(mut latest) = self.preview_latest.lock() {
                    *latest = Some(request);
                    AudioPrepEnqueueResult::Accepted
                } else {
                    AudioPrepEnqueueResult::Disconnected
                }
            }
            Err(mpsc::TrySendError::Disconnected(_)) => AudioPrepEnqueueResult::Disconnected,
        }
    }

    fn next_sequence(&self) -> u64 {
        self.next_sequence.fetch_add(1, Ordering::Relaxed)
    }

    fn try_send(&self, request: AudioControlRequest) -> AudioPrepEnqueueResult {
        match self.tx.try_send(request) {
            Ok(()) => AudioPrepEnqueueResult::Accepted,
            Err(mpsc::TrySendError::Full(_)) => AudioPrepEnqueueResult::Full,
            Err(mpsc::TrySendError::Disconnected(_)) => AudioPrepEnqueueResult::Disconnected,
        }
    }

    fn commit_owner_generation(
        &self,
        request: &AudioControlRequest,
        event: &rodio_engine_source::EngineEvent,
    ) {
        if let Ok(mut generations) = self.generations.lock() {
            match request {
                AudioControlRequest::InstrumentSlot {
                    instrument_slot,
                    generation,
                    ..
                } => {
                    generations.instrument[*instrument_slot] = *generation;
                    if matches!(
                        event,
                        rodio_engine_source::EngineEvent::SetPreparedInstrumentOwner {
                            sample_bank: Some(_),
                            ..
                        }
                    ) {
                        generations.sample[*instrument_slot] = *generation;
                    }
                }
                AudioControlRequest::FxBusSlot {
                    bus_index,
                    slot_index,
                    generation,
                    ..
                } => generations.fx_bus[*bus_index][*slot_index] = *generation,
                AudioControlRequest::GlobalFxSlot {
                    slot_index,
                    generation,
                    ..
                } => generations.global_fx[*slot_index] = *generation,
                AudioControlRequest::FullConfig { .. } | AudioControlRequest::SamplePreview(_) => {}
            }
        }
    }
}

#[cfg(test)]
fn handle_full_config_request(
    request: AudioControlRequest,
    rx: &std::sync::mpsc::Receiver<AudioControlRequest>,
    pending: &mut VecDeque<AudioControlRequest>,
    engine_tx: &rodio_engine_source::EngineEventSender,
    result_tx: &std::sync::mpsc::Sender<HostMessage>,
    state: &DesktopAudioPrepState,
) {
    audio_prep_worker::handle_full_config_request(
        request, rx, pending, engine_tx, result_tx, state,
    );
}

#[cfg(test)]
#[path = "audio_prep_service_tests.rs"]
mod extra_tests;
#[cfg(test)]
#[path = "audio_prep_preview_tests.rs"]
mod preview_tests;
