#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use super::cpal_audio_output::build_cpal_stream;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use super::cpal_audio_output::build_orange_cpal_stream;
use super::cpal_audio_output::BuiltAudioStream;
use super::{AudioSink, RecordingTapState};
use crate::audio::default_pi_instruments;
use crate::audio_priority::qualify_callback_scheduler;
use crate::audio_route::RouteOpenError;
use crate::audio_stream_health::AudioStreamHealth;
use cpal::traits::StreamTrait;
use cpal::Stream;
use realtime_engine::synth::{prepare_instruments_config, DEFAULT_AUDIO_SAMPLE_RATE};
#[cfg(test)]
use rodio_engine_source::EngineEventReceiver;
use rodio_engine_source::{event_queue, EngineEvent, EngineEventSender};
use std::time::Duration;

pub(super) const CALLBACK_SCHEDULING_STARTUP_TIMEOUT: Duration = Duration::from_millis(250);
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
const USB_AUDIO_STARTUP_FAULT_GRACE: Duration = Duration::from_millis(250);

pub(crate) struct OpenedAudioSink {
    pub(crate) engine_tx: EngineEventSender,
    pub(crate) _stream: Option<Stream>,
    pub(crate) health: AudioStreamHealth,
    #[cfg(test)]
    pub(crate) _test_engine_rx: Option<std::sync::Arc<std::sync::Mutex<EngineEventReceiver>>>,
}

pub(super) type AudioSinkOpener = fn(
    Option<u32>,
    AudioSink,
    Option<RecordingTapState>,
) -> Result<OpenedAudioSink, RouteOpenError>;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(super) fn open_audio_sink(
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    recording_tap: Option<RecordingTapState>,
) -> Result<OpenedAudioSink, RouteOpenError> {
    let (engine_tx, engine_rx) = event_queue();
    let health = if sink == AudioSink::Jack {
        AudioStreamHealth::new(format!("{sink:?}"))
    } else {
        AudioStreamHealth::optional(format!("{sink:?}"))
    };
    let built = build_cpal_stream(
        engine_rx,
        output_buffer_frames,
        sink,
        recording_tap,
        health.clone(),
    )?;
    let BuiltAudioStream { stream, scheduler } = built;
    stream
        .play()
        .map_err(super::cpal_audio_output::map_play_stream_error)?;
    if let Err(error) = qualify_callback_scheduler(
        sink.scheduler_label(),
        &scheduler,
        CALLBACK_SCHEDULING_STARTUP_TIMEOUT,
    ) {
        eprintln!("{error}");
    }
    if sink != AudioSink::Jack {
        std::thread::sleep(USB_AUDIO_STARTUP_FAULT_GRACE);
        if health.is_faulted() {
            return Err(RouteOpenError::Fault(format!(
                "{sink:?} audio stream entered a high-rate error loop"
            )));
        }
    }
    engine_tx
        .send(EngineEvent::SetPreparedInstruments(
            prepare_instruments_config(default_pi_instruments(), DEFAULT_AUDIO_SAMPLE_RATE),
        ))
        .map_err(|error| RouteOpenError::Fault(error.to_string()))?;
    Ok(OpenedAudioSink {
        engine_tx,
        _stream: Some(stream),
        health,
        #[cfg(test)]
        _test_engine_rx: None,
    })
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) fn open_orange_audio_sink(
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    recording_tap: Option<RecordingTapState>,
) -> Result<OpenedAudioSink, RouteOpenError> {
    let health = match sink {
        AudioSink::Jack => AudioStreamHealth::new("Jack".into()),
        AudioSink::Usb => AudioStreamHealth::optional("UAC2Gadget".into()),
        AudioSink::Hdmi => AudioStreamHealth::optional("HDMI".into()),
    };
    open_orange_audio_sink_with_health(output_buffer_frames, sink, health, recording_tap)
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) fn open_orange_audio_sink_with_health(
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    health: AudioStreamHealth,
    recording_tap: Option<RecordingTapState>,
) -> Result<OpenedAudioSink, RouteOpenError> {
    let (engine_tx, engine_rx) = event_queue();
    let built = build_orange_cpal_stream(
        engine_rx,
        output_buffer_frames,
        sink,
        recording_tap,
        health.clone(),
    )?;
    let BuiltAudioStream { stream, scheduler } = built;
    stream
        .play()
        .map_err(super::cpal_audio_output::map_play_stream_error)?;
    qualify_callback_scheduler(
        sink.scheduler_label(),
        &scheduler,
        CALLBACK_SCHEDULING_STARTUP_TIMEOUT,
    )
    .map_err(|error| RouteOpenError::Fault(error.to_string()))?;
    engine_tx
        .send(EngineEvent::SetPreparedInstruments(
            prepare_instruments_config(default_pi_instruments(), DEFAULT_AUDIO_SAMPLE_RATE),
        ))
        .map_err(|error| RouteOpenError::Fault(error.to_string()))?;
    Ok(OpenedAudioSink {
        engine_tx,
        _stream: Some(stream),
        health,
        #[cfg(test)]
        _test_engine_rx: None,
    })
}

pub(super) fn recordings_dir() -> std::path::PathBuf {
    crate::main_paths::default_recordings_dir()
}
