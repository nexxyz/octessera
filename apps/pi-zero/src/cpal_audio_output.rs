use super::audio_stream_lifecycle::{
    AudioStreamBuildError, AudioStreamLifecycle, AudioStreamShutdownError,
    AudioStreamShutdownReport, PlayableAudioStream,
};
use super::cpal_audio_callback::{fill_callback_with_scheduler, CallbackSource};
use super::AudioSink;
use super::RecordingTapState;
use crate::audio_priority::CallbackSchedulingHandle;
use crate::audio_route::RouteOpenError;
use crate::audio_stream_health::AudioStreamHealth;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, Stream, StreamConfig};
use platform_core::AUDIO_OUTPUT_BUFFER_FRAMES;
use realtime_engine::synth::DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio_engine_source::{
    AudioLoadStatusSender, EngineEventReceiver, EngineSource, EngineSourceWorkerShutdownOwner,
};

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
const DEFAULT_OUTPUT_BUFFER_FRAMES: u32 = AUDIO_OUTPUT_BUFFER_FRAMES as u32;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
/// The shipped direct-CPAL ALSA profile default for Orange audio.
pub(super) const ORANGE_DEFAULT_OUTPUT_BUFFER_FRAMES: u32 = AUDIO_OUTPUT_BUFFER_FRAMES as u32;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const ORANGE_BUFFER_QUALIFICATION_STAGES: &[u32] = &[1024, 512, 256];
const MIN_OUTPUT_BUFFER_FRAMES: u32 = 32;
const MAX_OUTPUT_BUFFER_FRAMES: u32 = 2048;

impl PlayableAudioStream for Stream {
    type Error = cpal::PlayStreamError;

    fn play(&self) -> Result<(), Self::Error> {
        StreamTrait::play(self)
    }
}

pub(super) struct BuiltAudioStream {
    lifecycle: AudioStreamLifecycle<Stream, EngineSourceWorkerShutdownOwner>,
    pub(super) scheduler: CallbackSchedulingHandle,
}

impl BuiltAudioStream {
    pub(super) fn play(&self) -> Result<(), cpal::PlayStreamError> {
        self.lifecycle.play()
    }

    pub(super) fn teardown(self) -> Result<AudioStreamShutdownReport, AudioStreamShutdownError> {
        self.lifecycle.teardown()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AudioSourceExecutionMode {
    Inline,
    PersistentTwoWorkers,
}

struct StreamBuildOptions {
    sink: AudioSink,
    execution_mode: AudioSourceExecutionMode,
    recording_tap: Option<RecordingTapState>,
    stream_health: AudioStreamHealth,
    load_tx: Option<AudioLoadStatusSender>,
}

pub(super) fn build_engine_source(
    engine_rx: EngineEventReceiver,
    sample_rate: u32,
    execution_mode: AudioSourceExecutionMode,
    load_tx: Option<AudioLoadStatusSender>,
) -> Result<(EngineSource, Option<EngineSourceWorkerShutdownOwner>), RouteOpenError> {
    match execution_mode {
        AudioSourceExecutionMode::Inline => Ok((EngineSource::new(engine_rx, sample_rate), None)),
        AudioSourceExecutionMode::PersistentTwoWorkers => {
            let block_frames =
                EngineSource::resolve_block_frames(DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES);
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            let result = EngineSource::with_persistent_workers_with_hook(
                engine_rx,
                sample_rate,
                block_frames,
                load_tx,
                crate::audio_priority::orange_worker_start_hook,
            );
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            let result = EngineSource::with_persistent_workers(
                engine_rx,
                sample_rate,
                block_frames,
                load_tx,
            );
            result
                .map(|(source, owner)| (source, Some(owner)))
                .map_err(|error| {
                    RouteOpenError::Fault(format!("persistent audio setup failed: {error:?}"))
                })
        }
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(super) fn probe_cpal_sink(sink: AudioSink) -> Result<(), RouteOpenError> {
    ensure_connector(sink)?;
    let device = cpal::alsa_exact_output_device(raspberry_pcm_name(sink))
        .map_err(|error| RouteOpenError::Fault(error.to_string()))?;
    let supported = device
        .default_output_config()
        .map_err(map_default_config_error)?;
    if supported.channels() != 2
        || supported.sample_rate().0 != DEFAULT_AUDIO_SAMPLE_RATE
        || !matches!(
            supported.sample_format(),
            SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
        )
    {
        return Err(RouteOpenError::Unsupported(format!(
            "{sink:?} audio device lacks project stereo format"
        )));
    }
    Ok(())
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(super) fn build_cpal_stream(
    engine_rx: EngineEventReceiver,
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    recording_tap: Option<RecordingTapState>,
    stream_health: AudioStreamHealth,
    execution_mode: AudioSourceExecutionMode,
) -> Result<BuiltAudioStream, RouteOpenError> {
    let host = cpal::default_host();
    let device = select_output_device(&host, sink)?;
    let supported = device
        .default_output_config()
        .map_err(map_default_config_error)?;
    let mut config: StreamConfig = supported.config();
    config.channels = 2;
    config.sample_rate = cpal::SampleRate(DEFAULT_AUDIO_SAMPLE_RATE);
    config.buffer_size = output_buffer_size(output_buffer_frames);
    let options = StreamBuildOptions {
        sink,
        execution_mode,
        recording_tap,
        stream_health,
        load_tx: None,
    };
    match supported.sample_format() {
        SampleFormat::F32 => build_stream_with_mode::<f32>(&device, &config, engine_rx, options),
        SampleFormat::I16 => build_stream_with_mode::<i16>(&device, &config, engine_rx, options),
        SampleFormat::U16 => build_stream_with_mode::<u16>(&device, &config, engine_rx, options),
        format => Err(RouteOpenError::Unsupported(format!(
            "unsupported audio sample format: {format:?}"
        ))),
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) fn build_orange_cpal_stream(
    engine_rx: EngineEventReceiver,
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    recording_tap: Option<RecordingTapState>,
    stream_health: AudioStreamHealth,
    execution_mode: AudioSourceExecutionMode,
    load_tx: Option<AudioLoadStatusSender>,
) -> Result<BuiltAudioStream, RouteOpenError> {
    ensure_connector(sink)?;
    let device = match sink {
        AudioSink::Jack => crate::orange_audio::select_orange_output_device()?,
        AudioSink::Usb => crate::orange_audio::select_orange_uac2_output_device()?,
        AudioSink::Hdmi => crate::orange_audio::select_orange_hdmi_output_device()?,
    };
    let (sample_format, mut config) = crate::orange_audio::select_orange_stream_config(&device)?;
    let output_buffer_frames = orange_output_buffer_frames(output_buffer_frames);
    config.buffer_size = BufferSize::Fixed(output_buffer_frames);
    let load_tx = (sink == AudioSink::Jack
        && matches!(
            execution_mode,
            AudioSourceExecutionMode::PersistentTwoWorkers
        ))
    .then_some(load_tx)
    .flatten();
    let options = StreamBuildOptions {
        sink,
        execution_mode,
        recording_tap,
        stream_health,
        load_tx,
    };
    match sample_format {
        SampleFormat::F32 => build_stream_with_mode::<f32>(&device, &config, engine_rx, options),
        SampleFormat::I16 => build_stream_with_mode::<i16>(&device, &config, engine_rx, options),
        SampleFormat::U16 => build_stream_with_mode::<u16>(&device, &config, engine_rx, options),
        format => Err(RouteOpenError::Unsupported(format!(
            "unsupported Orange audio sample format: {format:?}"
        ))),
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) fn probe_cpal_sink(sink: AudioSink) -> Result<(), RouteOpenError> {
    ensure_connector(sink)?;
    let device = match sink {
        AudioSink::Jack => crate::orange_audio::select_orange_output_device()?,
        AudioSink::Usb => crate::orange_audio::select_orange_uac2_output_device()?,
        AudioSink::Hdmi => crate::orange_audio::select_orange_hdmi_output_device()?,
    };
    crate::orange_audio::select_orange_stream_config(&device).map(|_| ())
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn select_output_device(
    host: &cpal::Host,
    sink: AudioSink,
) -> Result<cpal::Device, RouteOpenError> {
    let _ = host;
    ensure_connector(sink)?;
    cpal::alsa_exact_output_device(raspberry_pcm_name(sink))
        .map_err(|error| RouteOpenError::Fault(error.to_string()))
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn raspberry_pcm_name(sink: AudioSink) -> &'static str {
    match sink {
        AudioSink::Jack => cpal::ALSA_RASPBERRY_JACK_PCM,
        AudioSink::Usb => cpal::ALSA_RASPBERRY_USB_PCM,
        AudioSink::Hdmi => cpal::ALSA_RASPBERRY_HDMI_PCM,
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    source: EngineSource,
    shutdown_owner: Option<EngineSourceWorkerShutdownOwner>,
    sink: AudioSink,
    recording_tap: Option<RecordingTapState>,
    stream_health: AudioStreamHealth,
) -> Result<BuiltAudioStream, RouteOpenError>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let scheduler = callback_scheduler_for_sink(sink);
    let report_worker_health = shutdown_owner.is_some();
    let callback_scheduler = scheduler.clone();
    let callback_health = stream_health.clone();
    let mut worker_health_reported = false;
    let (mut callback_source, retirement_waiter) =
        CallbackSource::new(source, shutdown_owner.is_some());
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            fill_callback_with_scheduler(
                data,
                &mut callback_source,
                recording_tap.as_ref(),
                &callback_health,
                report_worker_health,
                &mut worker_health_reported,
                &callback_scheduler,
            );
        },
        move |error| stream_health.log(error),
        None,
    );
    let lifecycle =
        AudioStreamLifecycle::from_build_result(stream, shutdown_owner, retirement_waiter)
            .map_err(|error| match error {
                AudioStreamBuildError::Stream(error) => map_build_stream_error(error),
                AudioStreamBuildError::Shutdown(status) => map_shutdown_error(status),
            })?;
    Ok(BuiltAudioStream {
        lifecycle,
        scheduler,
    })
}

pub(super) fn callback_scheduler_for_sink(sink: AudioSink) -> CallbackSchedulingHandle {
    if cfg!(feature = "hardware-orange-pi-zero-2w") && sink == AudioSink::Jack {
        CallbackSchedulingHandle::new_orange_jack()
    } else {
        CallbackSchedulingHandle::new(crate::audio_priority::callback_priority())
    }
}

fn build_stream_with_mode<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    engine_rx: EngineEventReceiver,
    options: StreamBuildOptions,
) -> Result<BuiltAudioStream, RouteOpenError>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let StreamBuildOptions {
        sink,
        execution_mode,
        recording_tap,
        stream_health,
        load_tx,
    } = options;
    let (source, shutdown_owner) =
        build_engine_source(engine_rx, config.sample_rate.0, execution_mode, load_tx)?;
    build_stream::<T>(
        device,
        config,
        source,
        shutdown_owner,
        sink,
        recording_tap,
        stream_health,
    )
}

fn ensure_connector(sink: AudioSink) -> Result<(), RouteOpenError> {
    if sink == AudioSink::Hdmi {
        crate::hdmi_connector::HdmiConnectorProbe::fixed().require_connected()?;
    }
    Ok(())
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(super) fn map_default_config_error(error: cpal::DefaultStreamConfigError) -> RouteOpenError {
    match error {
        cpal::DefaultStreamConfigError::DeviceNotAvailable => RouteOpenError::Disconnected,
        cpal::DefaultStreamConfigError::DeviceBusy => RouteOpenError::Busy,
        cpal::DefaultStreamConfigError::StreamTypeNotSupported => {
            RouteOpenError::Unsupported(error.to_string())
        }
        cpal::DefaultStreamConfigError::BackendSpecific { .. } => {
            RouteOpenError::Fault(error.to_string())
        }
    }
}

pub(super) fn map_build_stream_error(error: cpal::BuildStreamError) -> RouteOpenError {
    match error {
        cpal::BuildStreamError::DeviceNotAvailable => RouteOpenError::Disconnected,
        cpal::BuildStreamError::DeviceBusy => RouteOpenError::Busy,
        cpal::BuildStreamError::StreamConfigNotSupported
        | cpal::BuildStreamError::InvalidArgument => RouteOpenError::Unsupported(error.to_string()),
        cpal::BuildStreamError::StreamIdOverflow
        | cpal::BuildStreamError::BackendSpecific { .. } => {
            RouteOpenError::Fault(error.to_string())
        }
    }
}

pub(super) fn map_play_stream_error(error: cpal::PlayStreamError) -> RouteOpenError {
    match error {
        cpal::PlayStreamError::DeviceNotAvailable => RouteOpenError::Disconnected,
        cpal::PlayStreamError::DeviceBusy => RouteOpenError::Busy,
        cpal::PlayStreamError::Unsupported(message) => RouteOpenError::Unsupported(message),
        cpal::PlayStreamError::Fault(message) => RouteOpenError::Fault(message),
        cpal::PlayStreamError::BackendSpecific { .. } => RouteOpenError::Fault(error.to_string()),
    }
}

pub(super) fn map_shutdown_error(status: AudioStreamShutdownError) -> RouteOpenError {
    RouteOpenError::Fault(format!("audio worker teardown failed: {status:?}"))
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn output_buffer_size(configured_frames: Option<u32>) -> BufferSize {
    BufferSize::Fixed(resolve_output_buffer_frames(
        std::env::var("OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES")
            .ok()
            .as_deref(),
        configured_frames,
        DEFAULT_OUTPUT_BUFFER_FRAMES,
    ))
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn orange_output_buffer_frames(configured_frames: Option<u32>) -> u32 {
    debug_assert_eq!(ORANGE_BUFFER_QUALIFICATION_STAGES, &[1024, 512, 256]);
    resolve_output_buffer_frames(
        std::env::var("OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES")
            .ok()
            .as_deref(),
        configured_frames,
        ORANGE_DEFAULT_OUTPUT_BUFFER_FRAMES,
    )
}

pub(super) fn resolve_output_buffer_frames(
    env_value: Option<&str>,
    configured_frames: Option<u32>,
    default_frames: u32,
) -> u32 {
    env_value
        .and_then(|value| value.parse::<u32>().ok())
        .or(configured_frames)
        .unwrap_or(default_frames)
        .clamp(MIN_OUTPUT_BUFFER_FRAMES, MAX_OUTPUT_BUFFER_FRAMES)
}
