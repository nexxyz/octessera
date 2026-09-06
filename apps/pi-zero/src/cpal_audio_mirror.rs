use super::super::audio_stream_lifecycle::{AudioStreamBuildError, AudioStreamLifecycle};
use super::super::cpal_audio_callback::{
    fill_mirror_callback_with_scheduler, MirrorCallbackSource,
};
use super::super::AudioSink;
use super::{
    callback_scheduler_for_sink, map_build_stream_error, map_shutdown_error, BuiltAudioStream,
};
use crate::audio_stream_health::AudioStreamHealth;
use cpal::traits::DeviceTrait;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use cpal::BufferSize;
use cpal::{SampleFormat, StreamConfig};
use rodio_engine_source::PcmMirrorConsumer;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use super::{output_buffer_size, select_output_device};

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn build_cpal_mirror_stream(
    output_buffer_frames: Option<u32>,
    sink: AudioSink,
    consumer: PcmMirrorConsumer,
    stream_health: AudioStreamHealth,
) -> Result<BuiltAudioStream, crate::audio_route::RouteOpenError> {
    let host = cpal::default_host();
    let device = select_output_device(&host, sink)?;
    let supported = device
        .default_output_config()
        .map_err(super::map_default_config_error)?;
    let mut config: StreamConfig = supported.config();
    config.channels = 2;
    config.sample_rate = cpal::SampleRate(super::DEFAULT_AUDIO_SAMPLE_RATE);
    config.buffer_size = output_buffer_size(output_buffer_frames);
    match supported.sample_format() {
        SampleFormat::F32 => {
            build_mirror_stream::<f32>(&device, &config, consumer, sink, stream_health)
        }
        SampleFormat::I16 => {
            build_mirror_stream::<i16>(&device, &config, consumer, sink, stream_health)
        }
        SampleFormat::U16 => {
            build_mirror_stream::<u16>(&device, &config, consumer, sink, stream_health)
        }
        format => Err(crate::audio_route::RouteOpenError::Unsupported(format!(
            "unsupported audio sample format: {format:?}"
        ))),
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) fn build_orange_cpal_mirror_stream(
    profile: super::OrangeAudioProfile,
    sink: AudioSink,
    consumer: PcmMirrorConsumer,
    stream_health: AudioStreamHealth,
) -> Result<BuiltAudioStream, crate::audio_route::RouteOpenError> {
    super::ensure_connector(sink)?;
    let device = match sink {
        AudioSink::Jack => crate::orange_audio::select_orange_output_device()?,
        AudioSink::Usb => crate::orange_audio::select_orange_uac2_output_device()?,
        AudioSink::Hdmi => crate::orange_audio::select_orange_hdmi_output_device()?,
    };
    let (sample_format, mut config) = crate::orange_audio::select_orange_stream_config(&device)?;
    config.buffer_size = BufferSize::Fixed(profile.output_buffer_frames);
    match sample_format {
        SampleFormat::F32 => {
            build_mirror_stream::<f32>(&device, &config, consumer, sink, stream_health)
        }
        SampleFormat::I16 => {
            build_mirror_stream::<i16>(&device, &config, consumer, sink, stream_health)
        }
        SampleFormat::U16 => {
            build_mirror_stream::<u16>(&device, &config, consumer, sink, stream_health)
        }
        format => Err(crate::audio_route::RouteOpenError::Unsupported(format!(
            "unsupported Orange audio sample format: {format:?}"
        ))),
    }
}

fn build_mirror_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    consumer: PcmMirrorConsumer,
    sink: AudioSink,
    stream_health: AudioStreamHealth,
) -> Result<BuiltAudioStream, crate::audio_route::RouteOpenError>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let scheduler = callback_scheduler_for_sink(sink);
    let callback_scheduler = scheduler.clone();
    let mut callback_source = MirrorCallbackSource::new(consumer);
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            fill_mirror_callback_with_scheduler(data, &mut callback_source, &callback_scheduler);
        },
        move |error| stream_health.log(error),
        None,
    );
    let lifecycle = AudioStreamLifecycle::from_build_result(stream, None, None).map_err(
        |error| match error {
            AudioStreamBuildError::Stream(error) => map_build_stream_error(error),
            AudioStreamBuildError::Shutdown(status) => map_shutdown_error(status),
        },
    )?;
    Ok(BuiltAudioStream {
        lifecycle,
        scheduler,
    })
}
