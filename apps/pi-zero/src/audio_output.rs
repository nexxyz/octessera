use super::{default_pi_instruments, AudioControlRequest, AudioService};
use crate::audio_hotplug::{default_replay_events, register_sink};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::audio_hotplug::{
    has_sink, recovery_enabled, remove_sink, replay_to_sink, startup_sinks, usb_uses_recording_tap,
    ReplayCache, SinkSender,
};
use crate::audio_stream_health::AudioStreamHealth;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) use crate::audio_stream_health::AudioStreamStatus as OrangeDacStatus;
mod audio_sink;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use audio_sink::audio_sinks;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) use audio_sink::orange_audio_sinks;
pub(crate) use audio_sink::AudioSink;
#[path = "cpal_audio_output.rs"]
mod cpal_audio_output;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_audio_recovery;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::recording::RecordingTap;
use crate::usb_config::UsbAudioOut;
use cpal::traits::StreamTrait;
use cpal::Stream;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use cpal_audio_output::build_cpal_stream;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use cpal_audio_output::build_orange_cpal_stream;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use orange_audio_recovery::{OrangeRecoveryController, OrangeRecoveryWorker};
use playback_runtime::HostMessage;
use realtime_engine::synth::{prepare_instruments_config, DEFAULT_AUDIO_SAMPLE_RATE};
use rodio_engine_source::{event_queue, EngineEvent, EngineEventSender};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::sync::RwLock;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CALLBACK_SCHEDULING_STARTUP_TIMEOUT: Duration = Duration::from_millis(250);
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
const USB_AUDIO_STARTUP_FAULT_GRACE: Duration = Duration::from_millis(250);
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
const USB_AUDIO_RECOVERY_INTERVAL: Duration = Duration::from_secs(2);
pub struct AudioManager {
    _streams: Vec<Stream>,
    service: AudioService,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    required_health: Option<AudioStreamHealth>,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    orange_dac_recovery: OrangeRecoveryController,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    _orange_usb_recovery: Option<OrangeRecoveryWorker>,
}

#[derive(Clone, Copy)]
enum AudioOpenPolicy {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    Usb(UsbAudioOut),
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    Orange(UsbAudioOut),
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum OrangeAudioInitError {
    UsbUnavailable,
    Open(String),
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
impl std::fmt::Display for OrangeAudioInitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UsbUnavailable => write!(
                formatter,
                "Orange runtimeConfig.usb.audioOut=usb is unavailable; internal DAC remains required"
            ),
            Self::Open(error) => formatter.write_str(error),
        }
    }
}

fn required_sink(policy: AudioOpenPolicy, sink: AudioSink) -> bool {
    match policy {
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        AudioOpenPolicy::Usb(UsbAudioOut::Jack) => sink == AudioSink::Jack,
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        AudioOpenPolicy::Usb(UsbAudioOut::Usb | UsbAudioOut::Both) => false,
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        AudioOpenPolicy::Orange(_) => sink == AudioSink::InternalDac,
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
struct ManagedUsbStream {
    _stream: Stream,
    health: AudioStreamHealth,
}

struct OpenedAudioSink {
    engine_tx: EngineEventSender,
    _stream: Option<Stream>,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    health: AudioStreamHealth,
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
type RecordingTapState = Arc<RwLock<Option<RecordingTap>>>;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
type RecordingTapState = ();

type AudioSinkOpener =
    fn(Option<u32>, AudioSink, Option<RecordingTapState>) -> Result<OpenedAudioSink, String>;

impl AudioManager {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub fn new(output_buffer_frames: Option<u32>, audio_out: UsbAudioOut) -> Result<Self, String> {
        Self::new_with_opener(
            output_buffer_frames,
            startup_sinks(audio_out),
            audio_out == UsbAudioOut::Both,
            AudioOpenPolicy::Usb(audio_out),
            open_audio_sink,
        )
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub fn new_orange(
        output_buffer_frames: Option<u32>,
        audio_out: UsbAudioOut,
    ) -> Result<Self, OrangeAudioInitError> {
        let sinks = orange_audio_sinks(audio_out)?
            .into_iter()
            .filter(|sink| *sink == AudioSink::InternalDac)
            .collect();
        Self::new_with_opener(
            output_buffer_frames,
            sinks,
            audio_out == UsbAudioOut::Both,
            AudioOpenPolicy::Orange(audio_out),
            open_orange_audio_sink,
        )
        .map_err(OrangeAudioInitError::Open)
    }

    fn new_with_opener(
        output_buffer_frames: Option<u32>,
        sinks: Vec<AudioSink>,
        allow_partial: bool,
        policy: AudioOpenPolicy,
        open_sink: AudioSinkOpener,
    ) -> Result<Self, String> {
        let (control_tx, control_rx) = mpsc::channel::<AudioControlRequest>();
        let (prep_result_tx, prep_result_rx) = mpsc::channel::<HostMessage>();
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let mut streams = Vec::new();
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let streams = Vec::new();
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let mut required_health = None;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let mut orange_dac_opened = None;
        let realtime_txs = Arc::new(Mutex::new(Vec::new()));
        let replay_events = Arc::new(Mutex::new(default_replay_events()));
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let recorder = Arc::new(Mutex::new(crate::recording::RecorderService::new(
            recordings_dir(),
        )));
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let recording_tap = Arc::new(RwLock::new(None));
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let mut recording_tap_claimed = false;
        for sink in sinks {
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            let uses_recording_tap = !recording_tap_claimed;
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            let tap = uses_recording_tap.then(|| recording_tap.clone());
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            let tap = None;
            match open_sink(output_buffer_frames, sink, tap) {
                Ok(opened) => {
                    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
                    if uses_recording_tap {
                        recording_tap_claimed = true;
                    }
                    #[cfg(feature = "hardware-orange-pi-zero-2w")]
                    {
                        register_sink(&realtime_txs, sink, opened.engine_tx.clone());
                        required_health = Some(opened.health.clone());
                        orange_dac_opened = Some(opened);
                    }
                    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
                    {
                        streams.push(
                            opened
                                ._stream
                                .expect("Raspberry audio stream must be present"),
                        );
                        register_sink(&realtime_txs, sink, opened.engine_tx);
                    }
                }
                Err(error) if allow_partial && !required_sink(policy, sink) => {
                    eprintln!("{sink:?} audio init failed: {error} (continuing with other sinks)");
                }
                Err(error) => return Err(error),
            }
        }
        let requires_stream = match policy {
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            AudioOpenPolicy::Usb(UsbAudioOut::Jack) => true,
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            AudioOpenPolicy::Usb(UsbAudioOut::Usb | UsbAudioOut::Both) => false,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            AudioOpenPolicy::Orange(audio_out) => audio_out != UsbAudioOut::Usb,
        };
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let no_required_stream = required_health.is_none();
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let no_required_stream = streams.is_empty();
        if no_required_stream && requires_stream {
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
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            recorder,
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            recording_tap: recording_tap.clone(),
        };
        crate::host_audio_prep::spawn_audio_control_worker(
            control_rx,
            service.clone(),
            prep_result_tx,
        );
        match policy {
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            AudioOpenPolicy::Usb(audio_out) if recovery_enabled(audio_out) => {
                spawn_usb_recovery_worker(
                    output_buffer_frames,
                    realtime_txs.clone(),
                    replay_events.clone(),
                    recording_tap.clone(),
                    audio_out,
                );
            }
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            AudioOpenPolicy::Usb(_) => {}
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            AudioOpenPolicy::Orange(_) => {}
        }
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let orange_dac_recovery = OrangeRecoveryController::new_required(
            orange_dac_opened
                .ok_or_else(|| "Orange internal DAC stream was not opened".to_string())?,
            output_buffer_frames,
            realtime_txs.clone(),
            replay_events.clone(),
        );
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let orange_usb_recovery = if matches!(policy, AudioOpenPolicy::Orange(UsbAudioOut::Both)) {
            Some(OrangeRecoveryWorker::spawn(
                output_buffer_frames,
                realtime_txs.clone(),
                replay_events.clone(),
            ))
        } else {
            None
        };
        Ok(Self {
            _streams: streams,
            service,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            required_health,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            orange_dac_recovery,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            _orange_usb_recovery: orange_usb_recovery,
        })
    }

    pub fn service(&self) -> AudioService {
        self.service.clone()
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub fn internal_dac_health(&self) -> Option<AudioStreamHealth> {
        self.required_health.clone()
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn required_dac_status(&self) -> OrangeDacStatus {
        self.orange_dac_recovery.status()
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
impl AudioManager {
    pub(crate) fn recover_audio_if_due(&mut self) {
        self.orange_dac_recovery.recover_if_due();
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn open_audio_sink(
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    recording_tap: Option<RecordingTapState>,
) -> Result<OpenedAudioSink, String> {
    let (engine_tx, engine_rx) = event_queue();
    let health = if sink == AudioSink::Usb {
        AudioStreamHealth::optional(format!("{sink:?}"))
    } else {
        AudioStreamHealth::new(format!("{sink:?}"))
    };
    let built = build_cpal_stream(
        engine_rx,
        output_buffer_frames,
        sink,
        recording_tap,
        health.clone(),
    )?;
    let cpal_audio_output::BuiltAudioStream { stream, scheduler } = built;
    stream
        .play()
        .map_err(|e| format!("failed to play {sink:?} audio stream: {e}"))?;
    if let Err(error) = crate::audio_priority::qualify_callback_scheduler(
        sink.scheduler_label(),
        &scheduler,
        CALLBACK_SCHEDULING_STARTUP_TIMEOUT,
    ) {
        eprintln!("{error}");
    }
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
    Ok(OpenedAudioSink {
        engine_tx,
        _stream: Some(stream),
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        health,
    })
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn open_orange_audio_sink(
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    _recording_tap: Option<RecordingTapState>,
) -> Result<OpenedAudioSink, String> {
    let health = match sink {
        AudioSink::InternalDac => AudioStreamHealth::new("InternalDac".into()),
        AudioSink::Usb => AudioStreamHealth::optional("UAC2Gadget".into()),
    };
    open_orange_audio_sink_with_health(output_buffer_frames, sink, health)
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn open_orange_audio_sink_with_health(
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    health: AudioStreamHealth,
) -> Result<OpenedAudioSink, String> {
    let (engine_tx, engine_rx) = event_queue();
    let built = build_orange_cpal_stream(engine_rx, output_buffer_frames, sink, health.clone())?;
    let cpal_audio_output::BuiltAudioStream { stream, scheduler } = built;
    stream
        .play()
        .map_err(|e| format!("failed to play Orange audio stream: {e}"))?;
    crate::audio_priority::qualify_callback_scheduler(
        sink.scheduler_label(),
        &scheduler,
        CALLBACK_SCHEDULING_STARTUP_TIMEOUT,
    )?;
    engine_tx
        .send(EngineEvent::SetPreparedInstruments(
            prepare_instruments_config(default_pi_instruments(), DEFAULT_AUDIO_SAMPLE_RATE),
        ))
        .map_err(|error| error.to_string())?;
    Ok(OpenedAudioSink {
        engine_tx,
        _stream: Some(stream),
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        health,
    })
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn open_managed_usb_sink(
    output_buffer_frames: Option<u32>,
    recording_tap: Option<RecordingTapState>,
) -> Result<(EngineEventSender, ManagedUsbStream), String> {
    let sink = AudioSink::Usb;
    let (engine_tx, engine_rx) = event_queue();
    let health = AudioStreamHealth::optional(format!("{sink:?}"));
    let built = build_cpal_stream(
        engine_rx,
        output_buffer_frames,
        sink,
        recording_tap,
        health.clone(),
    )?;
    let cpal_audio_output::BuiltAudioStream { stream, scheduler } = built;
    stream
        .play()
        .map_err(|e| format!("failed to play {sink:?} audio stream: {e}"))?;
    if let Err(error) = crate::audio_priority::qualify_callback_scheduler(
        sink.scheduler_label(),
        &scheduler,
        CALLBACK_SCHEDULING_STARTUP_TIMEOUT,
    ) {
        eprintln!("{error}");
    }
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

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
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

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn recordings_dir() -> PathBuf {
    std::env::var("OCTESSERA_PI_RECORDINGS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/home/pi/recordings"))
}

#[cfg(test)]
#[path = "audio_output_tests.rs"]
mod tests;
