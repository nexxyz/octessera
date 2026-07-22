use crate::audio_hotplug::{
    broadcast_event, default_replay_events, has_sink, is_replay_event, recovery_enabled,
    register_sink, remove_sink, replay_to_sink, startup_sinks, usb_uses_recording_tap, ReplayCache,
    SinkSender,
};
use crate::audio_stream_health::AudioStreamHealth;
use crate::recording::{RecorderService, RecordingTap};
use crate::usb_config::UsbAudioOut;
#[path = "audio_defaults.rs"]
mod audio_defaults;
#[path = "audio_error.rs"]
mod audio_error;
pub(crate) use audio_defaults::default_pi_instruments;
use audio_error::audio_queue_error;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, Stream, StreamConfig};
use playback_runtime::{HostMessage, RuntimeAdapterError};
use realtime_engine::synth::{prepare_instruments_config, DEFAULT_AUDIO_SAMPLE_RATE};
use rodio_engine_source::{
    event_queue, EngineEvent, EngineEventReceiver, EngineEventSender, EngineSource,
};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

const USB_AUDIO_STARTUP_FAULT_GRACE: Duration = Duration::from_millis(250);
const USB_AUDIO_RECOVERY_INTERVAL: Duration = Duration::from_secs(2);

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
    Dynamic(Box<EngineEvent>),
}

pub struct AudioManager {
    _streams: Vec<Stream>,
    service: AudioService,
}

struct ManagedUsbStream {
    _stream: Stream,
    health: AudioStreamHealth,
}

impl AudioManager {
    pub fn new(output_buffer_frames: Option<u32>, audio_out: UsbAudioOut) -> Result<Self, String> {
        let (control_tx, control_rx) = mpsc::channel::<AudioControlRequest>();
        let (prep_result_tx, prep_result_rx) = mpsc::channel::<HostMessage>();
        let mut streams = Vec::new();
        let realtime_txs = Arc::new(Mutex::new(Vec::new()));
        let replay_events = Arc::new(Mutex::new(default_replay_events()));
        let sinks = startup_sinks(audio_out);
        let recorder = Arc::new(Mutex::new(RecorderService::new(recordings_dir())));
        let recording_tap = Arc::new(RwLock::new(None));
        let mut recording_tap_claimed = false;
        for sink in sinks {
            let uses_recording_tap = !recording_tap_claimed;
            let tap = uses_recording_tap.then(|| recording_tap.clone());
            match open_audio_sink(output_buffer_frames, sink, tap.clone()) {
                Ok((engine_tx, stream)) => {
                    if uses_recording_tap {
                        recording_tap_claimed = true;
                    }
                    streams.push(stream);
                    register_sink(&realtime_txs, sink, engine_tx);
                }
                Err(error) if audio_out == UsbAudioOut::Both => {
                    eprintln!("{sink:?} audio init failed: {error} (continuing with other sinks)");
                }
                Err(error) => return Err(error),
            }
        }
        if streams.is_empty() && audio_out == UsbAudioOut::Jack {
            return Err("no requested audio outputs opened".into());
        }
        let service = AudioService {
            realtime_txs: realtime_txs.clone(),
            replay_events: replay_events.clone(),
            control_tx,
            config_revision: Arc::new(AtomicU64::new(0)),
            sample_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            sample_bank_signature: Arc::new(Mutex::new(String::new())),
            prep_result_rx: Arc::new(Mutex::new(prep_result_rx)),
            recorder,
            recording_tap: recording_tap.clone(),
        };
        crate::host_audio_prep::spawn_audio_control_worker(
            control_rx,
            service.clone(),
            prep_result_tx,
        );
        if recovery_enabled(audio_out) {
            spawn_usb_recovery_worker(
                output_buffer_frames,
                realtime_txs.clone(),
                replay_events.clone(),
                recording_tap.clone(),
                audio_out,
            );
        }
        Ok(Self {
            _streams: streams,
            service,
        })
    }

    pub fn service(&self) -> AudioService {
        self.service.clone()
    }
}

fn open_audio_sink(
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    recording_tap: Option<Arc<RwLock<Option<RecordingTap>>>>,
) -> Result<(EngineEventSender, Stream), String> {
    let (engine_tx, engine_rx) = event_queue();
    let health = AudioStreamHealth::new(format!("{sink:?}"));
    let stream = build_cpal_stream(
        engine_rx,
        output_buffer_frames,
        sink,
        recording_tap,
        health.clone(),
    )?;
    stream
        .play()
        .map_err(|e| format!("failed to play {sink:?} audio stream: {e}"))?;
    if sink == AudioSink::Usb {
        std::thread::sleep(USB_AUDIO_STARTUP_FAULT_GRACE);
        if health.is_faulted() {
            return Err("USB audio stream entered a high-rate error loop".into());
        }
    }
    engine_tx
        .send(EngineEvent::SetPreparedInstruments(
            prepare_instruments_config(default_pi_instruments(), DEFAULT_AUDIO_SAMPLE_RATE),
        ))
        .map_err(|error| error.to_string())?;
    Ok((engine_tx, stream))
}

fn open_managed_usb_sink(
    output_buffer_frames: Option<u32>,
    recording_tap: Option<Arc<RwLock<Option<RecordingTap>>>>,
) -> Result<(EngineEventSender, ManagedUsbStream), String> {
    let sink = AudioSink::Usb;
    let (engine_tx, engine_rx) = event_queue();
    let health = AudioStreamHealth::new(format!("{sink:?}"));
    let stream = build_cpal_stream(
        engine_rx,
        output_buffer_frames,
        sink,
        recording_tap,
        health.clone(),
    )?;
    stream
        .play()
        .map_err(|e| format!("failed to play {sink:?} audio stream: {e}"))?;
    std::thread::sleep(USB_AUDIO_STARTUP_FAULT_GRACE);
    if health.is_faulted() {
        return Err("USB audio stream entered a high-rate error loop".into());
    }
    Ok((
        engine_tx,
        ManagedUsbStream {
            _stream: stream,
            health,
        },
    ))
}

fn spawn_usb_recovery_worker(
    output_buffer_frames: Option<u32>,
    realtime_txs: Arc<Mutex<Vec<SinkSender>>>,
    replay_events: Arc<Mutex<ReplayCache>>,
    recording_tap: Arc<RwLock<Option<RecordingTap>>>,
    audio_out: UsbAudioOut,
) {
    std::thread::spawn(move || {
        let mut managed: Option<ManagedUsbStream> = None;
        loop {
            if managed
                .as_ref()
                .is_some_and(|stream| stream.health.is_faulted())
            {
                remove_sink(&realtime_txs, AudioSink::Usb);
                managed = None;
                eprintln!("USB audio stream faulted; waiting for gadget audio to return");
            }
            if !has_sink(&realtime_txs, AudioSink::Usb) {
                let tap = usb_uses_recording_tap(audio_out).then(|| recording_tap.clone());
                match open_managed_usb_sink(output_buffer_frames, tap) {
                    Ok((tx, stream)) => {
                        if let Err(error) = replay_to_sink(&tx, &replay_events) {
                            eprintln!("USB audio replay failed: {error}");
                            continue;
                        }
                        register_sink(&realtime_txs, AudioSink::Usb, tx);
                        stream.health.clear_faulted();
                        managed = Some(stream);
                        eprintln!("USB audio stream ready");
                    }
                    Err(error) => eprintln!("USB audio unavailable: {error}"),
                }
            }
            std::thread::sleep(USB_AUDIO_RECOVERY_INTERVAL);
        }
    });
}

fn build_cpal_stream(
    engine_rx: EngineEventReceiver,
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    recording_tap: Option<Arc<RwLock<Option<RecordingTap>>>>,
    stream_health: AudioStreamHealth,
) -> Result<Stream, String> {
    let host = cpal::default_host();
    let device = select_output_device(&host, sink)?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("failed to read default audio config: {e}"))?;
    let mut config: StreamConfig = supported.config();
    config.channels = 2;
    config.sample_rate = cpal::SampleRate(DEFAULT_AUDIO_SAMPLE_RATE);
    config.buffer_size = output_buffer_size(output_buffer_frames);
    let source = EngineSource::new(engine_rx, config.sample_rate.0);
    match supported.sample_format() {
        SampleFormat::F32 => {
            build_stream::<f32>(&device, &config, source, recording_tap, stream_health)
        }
        SampleFormat::I16 => {
            build_stream::<i16>(&device, &config, source, recording_tap, stream_health)
        }
        SampleFormat::U16 => {
            build_stream::<u16>(&device, &config, source, recording_tap, stream_health)
        }
        format => Err(format!("unsupported audio sample format: {format:?}")),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AudioSink {
    Jack,
    Usb,
}

#[cfg(test)]
pub(crate) fn audio_sinks(audio_out: UsbAudioOut) -> Vec<AudioSink> {
    match audio_out {
        UsbAudioOut::Jack => vec![AudioSink::Jack],
        UsbAudioOut::Usb => vec![AudioSink::Usb],
        UsbAudioOut::Both => vec![AudioSink::Jack, AudioSink::Usb],
    }
}

fn select_output_device(host: &cpal::Host, sink: AudioSink) -> Result<cpal::Device, String> {
    let env_name = match sink {
        AudioSink::Jack => "OCTESSERA_AUDIO_JACK_DEVICE",
        AudioSink::Usb => "OCTESSERA_AUDIO_USB_DEVICE",
    };
    let devices: Vec<_> = host
        .output_devices()
        .map_err(|e| format!("failed to enumerate audio output devices: {e}"))?
        .collect();
    if let Ok(needle) = std::env::var(env_name) {
        if let Some(device) = find_named_device(&devices, &needle) {
            return Ok(device);
        }
        return Err(format!(
            "audio device override {env_name}={needle:?} not found"
        ));
    }
    match sink {
        AudioSink::Jack => devices
            .iter()
            .find(|d| !is_usb_gadget_name(&device_name(d)))
            .cloned()
            .or_else(|| host.default_output_device())
            .ok_or_else(|| "no jack/default audio output device".to_string()),
        AudioSink::Usb => devices
            .iter()
            .find(|d| is_usb_gadget_name(&device_name(d)))
            .cloned()
            .ok_or_else(|| "no USB gadget audio output device".to_string()),
    }
}

fn find_named_device(devices: &[cpal::Device], needle: &str) -> Option<cpal::Device> {
    let needle = needle.to_ascii_lowercase();
    devices
        .iter()
        .find(|device| device_name(device).to_ascii_lowercase().contains(&needle))
        .cloned()
}

fn device_name(device: &cpal::Device) -> String {
    device.name().unwrap_or_else(|_| String::new())
}

fn is_usb_gadget_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("octessera")
        || name.contains("uac")
        || name.contains("gadget")
        || name.contains("usb audio")
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut source: EngineSource,
    recording_tap: Option<Arc<RwLock<Option<RecordingTap>>>>,
    stream_health: AudioStreamHealth,
) -> Result<Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                crate::audio_priority::configure_callback_thread();
                fill_output(data, &mut source, recording_tap.as_ref());
            },
            move |error| stream_health.log(error),
            None,
        )
        .map_err(|e| format!("failed to build audio output stream: {e}"))
}

fn fill_output<T>(
    data: &mut [T],
    source: &mut EngineSource,
    recording_tap: Option<&Arc<RwLock<Option<RecordingTap>>>>,
) where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    let recorded = recording_tap
        .and_then(|tap| tap.try_read().ok())
        .and_then(|tap| tap.as_ref().cloned());
    let mut recording_chunk = recorded
        .as_ref()
        .map(|_| crate::recording::RecordingChunk::new());
    for sample in data {
        let value = source.next().unwrap_or(0.0);
        if let (Some(tap), Some(chunk)) = (recorded.as_ref(), recording_chunk.as_mut()) {
            if !chunk.push(float_to_i16(value)) {
                tap.push_chunk(chunk.take());
                let _ = chunk.push(float_to_i16(value));
            }
        }
        *sample = T::from_sample(value);
    }
    if let (Some(tap), Some(chunk)) = (recorded.as_ref(), recording_chunk) {
        if !chunk.is_empty() {
            tap.push_chunk(chunk);
        }
    }
}

fn float_to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
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

fn recordings_dir() -> PathBuf {
    std::env::var("OCTESSERA_PI_RECORDINGS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/home/pi/recordings"))
}
