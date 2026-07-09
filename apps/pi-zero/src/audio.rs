use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, Stream, StreamConfig};
use realtime_engine::synth::{
    default_synth_config, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    DEFAULT_AUDIO_SAMPLE_RATE, DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use rodio_engine_source::{EngineEvent, EngineSource};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "linux")]
mod audio_priority {
    use std::cell::Cell;

    thread_local! {
        static PRIORITY_CONFIGURED: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn configure_callback_thread() {
        PRIORITY_CONFIGURED.with(|configured| {
            if configured.get() {
                return;
            }
            configured.set(true);
            let priority = std::env::var("OCTESSERA_AUDIO_THREAD_PRIORITY")
                .ok()
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(70)
                .clamp(1, 80);
            let params = libc::sched_param {
                sched_priority: priority,
            };
            let result = unsafe {
                libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_FIFO, &params)
            };
            if result != 0 {
                eprintln!("audio thread realtime priority unavailable: errno {result}");
            }
        });
    }
}

#[cfg(not(target_os = "linux"))]
mod audio_priority {
    pub(super) fn configure_callback_thread() {}
}

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
    _stream: Stream,
    service: AudioService,
}

impl AudioManager {
    pub fn new(output_buffer_frames: Option<u32>) -> Result<Self, String> {
        let (engine_tx, engine_rx) = mpsc::channel::<EngineEvent>();
        let (control_tx, control_rx) = mpsc::channel::<AudioControlRequest>();
        let stream = build_cpal_stream(engine_rx, output_buffer_frames)?;
        stream
            .play()
            .map_err(|e| format!("failed to play audio stream: {e}"))?;
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
            service,
        })
    }

    pub fn service(&self) -> AudioService {
        self.service.clone()
    }
}

fn build_cpal_stream(
    engine_rx: mpsc::Receiver<EngineEvent>,
    output_buffer_frames: Option<u32>,
) -> Result<Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no default audio output device".to_string())?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("failed to read default audio config: {e}"))?;
    let mut config: StreamConfig = supported.config();
    config.channels = 2;
    config.sample_rate = cpal::SampleRate(DEFAULT_AUDIO_SAMPLE_RATE);
    config.buffer_size = output_buffer_size(output_buffer_frames);
    let source = EngineSource::new(engine_rx, config.sample_rate.0);
    match supported.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(&device, &config, source),
        SampleFormat::I16 => build_stream::<i16>(&device, &config, source),
        SampleFormat::U16 => build_stream::<u16>(&device, &config, source),
        format => Err(format!("unsupported audio sample format: {format:?}")),
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut source: EngineSource,
) -> Result<Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                audio_priority::configure_callback_thread();
                fill_output(data, &mut source);
            },
            move |error| eprintln!("audio stream error: {error}"),
            None,
        )
        .map_err(|e| format!("failed to build audio output stream: {e}"))
}

fn fill_output<T>(data: &mut [T], source: &mut EngineSource)
where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    for sample in data {
        *sample = T::from_sample(source.next().unwrap_or(0.0));
    }
}

fn output_buffer_size(configured_frames: Option<u32>) -> BufferSize {
    let frames = std::env::var("OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .or(configured_frames)
        .unwrap_or(256);
    BufferSize::Fixed(frames.clamp(32, 2048))
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
