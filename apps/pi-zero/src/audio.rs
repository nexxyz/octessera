use realtime_engine::synth::{
    default_synth_config, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    DEFAULT_AUDIO_SAMPLE_RATE, DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use rodio::{OutputStream, Sink};
use rodio_engine_source::{EngineEvent, EngineSource};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AudioService {
    realtime_tx: Sender<EngineEvent>,
    pub control_tx: Sender<AudioControlRequest>,
    pub config_revision: Arc<AtomicU64>,
    pub sample_cache:
        Arc<Mutex<std::collections::HashMap<String, realtime_engine::synth::SampleBuffer>>>,
    pub sample_bank_signature: Arc<Mutex<String>>,
}

pub enum AudioControlRequest {
    FullConfig {
        revision: u64,
        config: Value,
        samples_dir: PathBuf,
    },
    Dynamic(EngineEvent),
}

pub struct AudioManager {
    _stream: OutputStream,
    _sink: Sink,
    service: AudioService,
}

impl AudioManager {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("failed to create audio stream: {e}"))?;
        let sink = Sink::try_new(&handle).map_err(|e| format!("failed to create sink: {e}"))?;
        let (engine_tx, engine_rx) = mpsc::channel::<EngineEvent>();
        let (control_tx, control_rx) = mpsc::channel::<AudioControlRequest>();
        sink.append(EngineSource::new(engine_rx, DEFAULT_AUDIO_SAMPLE_RATE));
        sink.play();
        let _ = engine_tx.send(EngineEvent::SetInstruments(default_pi_instruments()));
        let service = AudioService {
            realtime_tx: engine_tx.clone(),
            control_tx,
            config_revision: Arc::new(AtomicU64::new(0)),
            sample_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            sample_bank_signature: Arc::new(Mutex::new(String::new())),
        };
        crate::host_audio_prep::spawn_audio_control_worker(
            control_rx,
            engine_tx.clone(),
            service.clone(),
        );
        Ok(Self {
            _stream: stream,
            _sink: sink,
            service,
        })
    }

    pub fn service(&self) -> AudioService {
        self.service.clone()
    }
}

impl AudioService {
    pub fn send(&self, event: EngineEvent) -> Result<(), String> {
        self.control_tx
            .send(AudioControlRequest::Dynamic(event))
            .map_err(|e| format!("audio control send failed: {e}"))
    }

    pub fn send_realtime(&self, event: EngineEvent) -> Result<(), String> {
        self.realtime_tx
            .send(event)
            .map_err(|e| format!("audio realtime send failed: {e}"))
    }

    pub fn enqueue_full_config(
        &self,
        revision: u64,
        config: Value,
        samples_dir: PathBuf,
    ) -> Result<(), String> {
        self.control_tx
            .send(AudioControlRequest::FullConfig {
                revision,
                config,
                samples_dir,
            })
            .map_err(|e| format!("audio prep send failed: {e}"))
    }
}

fn default_pi_instruments() -> InstrumentsConfig {
    let synth = default_synth_config();
    InstrumentsConfig {
        instruments: (0..INSTRUMENT_SLOT_COUNT)
            .map(|idx| InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth,
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: idx.min(DEFAULT_PAN_POSITIONS - 1),
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}
