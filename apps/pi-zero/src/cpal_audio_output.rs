use super::AudioSink;
use super::RecordingTapState;
use crate::audio_priority::CallbackSchedulingHandle;
use crate::audio_route::RouteOpenError;
use crate::audio_stream_health::AudioStreamHealth;
use cpal::traits::DeviceTrait;
use cpal::{BufferSize, SampleFormat, Stream, StreamConfig};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio_engine_source::{EngineEventReceiver, EngineSource};

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
const DEFAULT_OUTPUT_BUFFER_FRAMES: u32 = 256;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
/// The shipped direct-CPAL ALSA profile default for Orange audio.
pub(super) const ORANGE_DEFAULT_OUTPUT_BUFFER_FRAMES: u32 = 256;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const ORANGE_BUFFER_QUALIFICATION_STAGES: &[u32] = &[1024, 512, 256];
const MIN_OUTPUT_BUFFER_FRAMES: u32 = 32;
const MAX_OUTPUT_BUFFER_FRAMES: u32 = 2048;

pub(super) struct BuiltAudioStream {
    pub(super) stream: Stream,
    pub(super) scheduler: CallbackSchedulingHandle,
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
    let engine_block_frames =
        EngineSource::resolve_block_frames(orange_engine_block_frames(output_buffer_frames));
    let source =
        EngineSource::with_block_frames(engine_rx, config.sample_rate.0, engine_block_frames);
    match sample_format {
        SampleFormat::F32 => {
            build_stream::<f32>(&device, &config, source, recording_tap, stream_health)
        }
        SampleFormat::I16 => {
            build_stream::<i16>(&device, &config, source, recording_tap, stream_health)
        }
        SampleFormat::U16 => {
            build_stream::<u16>(&device, &config, source, recording_tap, stream_health)
        }
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
    mut source: EngineSource,
    recording_tap: Option<RecordingTapState>,
    stream_health: AudioStreamHealth,
) -> Result<BuiltAudioStream, RouteOpenError>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let scheduler = CallbackSchedulingHandle::new(crate::audio_priority::configured_priority());
    let callback_scheduler = scheduler.clone();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                callback_scheduler.configure_callback_thread();
                fill_output(data, &mut source, recording_tap.as_ref());
            },
            move |error| stream_health.log(error),
            None,
        )
        .map(|stream| BuiltAudioStream { stream, scheduler })
        .map_err(map_build_stream_error)
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

fn fill_output<T>(
    data: &mut [T],
    source: &mut EngineSource,
    recording_tap: Option<&RecordingTapState>,
) where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    let recording_tap_guard = recording_tap.and_then(|tap| tap.try_read().ok());
    let recorded = recording_tap_guard
        .as_ref()
        .and_then(|tap| (**tap).as_ref());
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

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) fn orange_engine_block_frames(output_buffer_frames: u32) -> usize {
    (output_buffer_frames / 4) as usize
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
