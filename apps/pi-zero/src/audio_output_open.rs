use super::audio_stream_lifecycle::{AudioStreamShutdownError, AudioStreamShutdownReport};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use super::cpal_audio_output::build_cpal_stream;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use super::cpal_audio_output::build_orange_cpal_stream;
use super::cpal_audio_output::AudioSourceExecutionMode;
use super::cpal_audio_output::BuiltAudioStream;
use super::cpal_audio_output::EngineSourceOptions;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use super::cpal_audio_output::OrangeAudioProfile;
use super::{AudioSink, RecordingTapState};
use crate::audio::default_pi_instruments;
use crate::audio_priority::qualify_callback_scheduler;
use crate::audio_route::RouteOpenError;
use crate::audio_stream_health::AudioStreamHealth;
use realtime_engine::synth::{prepare_instruments_config, DEFAULT_AUDIO_SAMPLE_RATE};
#[cfg(test)]
use rodio_engine_source::EngineEventReceiver;
use rodio_engine_source::{
    event_queue, AudioLoadStatusSender, EngineEvent, EngineEventSender, PcmMirrorConsumer,
    PcmMirrorProducers,
};
use std::time::Duration;

pub(super) const CALLBACK_SCHEDULING_STARTUP_TIMEOUT: Duration = Duration::from_millis(250);
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
const USB_AUDIO_STARTUP_FAULT_GRACE: Duration = Duration::from_millis(250);

pub(crate) struct OpenedAudioSink {
    pub(crate) engine_tx: Option<EngineEventSender>,
    pub(crate) _stream: Option<Box<BuiltAudioStream>>,
    pub(crate) health: AudioStreamHealth,
    #[cfg(test)]
    pub(crate) _test_engine_rx: Option<std::sync::Arc<std::sync::Mutex<EngineEventReceiver>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AudioConstructionConfig {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    OutputBuffer(Option<u32>),
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    Orange(OrangeAudioProfile),
}

impl AudioConstructionConfig {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub(super) const fn raspberry(output_buffer_frames: Option<u32>) -> Self {
        Self::OutputBuffer(output_buffer_frames)
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(super) const fn orange(profile: OrangeAudioProfile) -> Self {
        Self::Orange(profile)
    }
}

pub(super) type AudioSinkOpener = fn(
    AudioConstructionConfig,
    AudioSink,
    Option<RecordingTapState>,
    Option<AudioLoadStatusSender>,
    PcmMirrorProducers,
    Option<PcmMirrorConsumer>,
) -> Result<OpenedAudioSink, RouteOpenError>;

pub(super) fn source_execution_mode(
    sink: AudioSink,
    config: AudioConstructionConfig,
) -> AudioSourceExecutionMode {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let _ = (sink, config);
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    if sink == AudioSink::Jack
        && matches!(
            config,
            AudioConstructionConfig::Orange(OrangeAudioProfile {
                optimization: playback_runtime::AudioOptimization::Capacity,
                ..
            })
        )
    {
        return AudioSourceExecutionMode::RoutingTree;
    }
    AudioSourceExecutionMode::Inline
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(super) fn open_audio_sink(
    config: AudioConstructionConfig,
    sink: AudioSink,
    recording_tap: Option<RecordingTapState>,
    _load_tx: Option<AudioLoadStatusSender>,
    mirror_producers: PcmMirrorProducers,
    mirror_consumer: Option<PcmMirrorConsumer>,
) -> Result<OpenedAudioSink, RouteOpenError> {
    let AudioConstructionConfig::OutputBuffer(output_buffer_frames) = config;
    let health = if sink == AudioSink::Jack {
        AudioStreamHealth::new(format!("{sink:?}"))
    } else {
        AudioStreamHealth::optional(format!("{sink:?}"))
    };
    let (built, engine_tx) = if sink == AudioSink::Jack {
        let (engine_tx, engine_rx) = event_queue();
        let built = build_cpal_stream(
            engine_rx,
            output_buffer_frames,
            sink,
            EngineSourceOptions {
                recording_tap,
                load_tx: None,
                mirror_producers,
            },
            health.clone(),
            source_execution_mode(sink, config),
        )?;
        (built, Some(engine_tx))
    } else {
        let consumer = mirror_consumer.ok_or_else(|| {
            RouteOpenError::Fault(format!("{sink:?} audio mirror is not configured"))
        })?;
        (
            super::cpal_audio_output::build_cpal_mirror_stream(
                output_buffer_frames,
                sink,
                consumer,
                health.clone(),
            )?,
            None,
        )
    };
    if let Err(error) = built.play() {
        if let Err(status) = built.teardown() {
            return Err(super::cpal_audio_output::map_shutdown_error(status));
        }
        return Err(super::cpal_audio_output::map_play_stream_error(error));
    }
    let built = if sink == AudioSink::Jack {
        let scheduler = built.scheduler.clone();
        qualify_jack_or_teardown(&scheduler, built, |built| built.teardown())?
    } else {
        qualify_callback_scheduler(
            sink.scheduler_label(),
            &built.scheduler,
            CALLBACK_SCHEDULING_STARTUP_TIMEOUT,
        )
        .map_err(RouteOpenError::Fault)?;
        built
    };
    if sink != AudioSink::Jack {
        std::thread::sleep(USB_AUDIO_STARTUP_FAULT_GRACE);
        if health.external_is_faulted() {
            return Err(RouteOpenError::Fault(format!(
                "{sink:?} audio stream entered a high-rate error loop"
            )));
        }
    }
    if let Some(engine_tx) = engine_tx.as_ref() {
        engine_tx
            .send(EngineEvent::SetPreparedInstruments(
                prepare_instruments_config(default_pi_instruments(), DEFAULT_AUDIO_SAMPLE_RATE),
            ))
            .map_err(|error| RouteOpenError::Fault(error.to_string()))?;
    }
    Ok(OpenedAudioSink {
        engine_tx,
        _stream: Some(Box::new(built)),
        health,
        #[cfg(test)]
        _test_engine_rx: None,
    })
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) fn open_orange_audio_sink(
    config: AudioConstructionConfig,
    sink: AudioSink,
    recording_tap: Option<RecordingTapState>,
    load_tx: Option<AudioLoadStatusSender>,
    mirror_producers: PcmMirrorProducers,
    mirror_consumer: Option<PcmMirrorConsumer>,
) -> Result<OpenedAudioSink, RouteOpenError> {
    let health = match sink {
        AudioSink::Jack => AudioStreamHealth::new("Jack".into()),
        AudioSink::Usb => AudioStreamHealth::optional("UAC2Gadget".into()),
        AudioSink::Hdmi => AudioStreamHealth::optional("HDMI".into()),
    };
    open_orange_audio_sink_with_health(
        config,
        sink,
        health,
        recording_tap,
        load_tx,
        mirror_producers,
        mirror_consumer,
    )
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) fn open_orange_audio_sink_with_health(
    config: AudioConstructionConfig,
    sink: AudioSink,
    health: AudioStreamHealth,
    recording_tap: Option<RecordingTapState>,
    load_tx: Option<AudioLoadStatusSender>,
    mirror_producers: PcmMirrorProducers,
    mirror_consumer: Option<PcmMirrorConsumer>,
) -> Result<OpenedAudioSink, RouteOpenError> {
    let AudioConstructionConfig::Orange(profile) = config;
    let (built, engine_tx) = if sink == AudioSink::Jack {
        let (engine_tx, engine_rx) = event_queue();
        let built = build_orange_cpal_stream(
            engine_rx,
            profile,
            sink,
            EngineSourceOptions {
                recording_tap,
                load_tx,
                mirror_producers,
            },
            health.clone(),
            source_execution_mode(sink, AudioConstructionConfig::Orange(profile)),
        )?;
        (built, Some(engine_tx))
    } else {
        let consumer = mirror_consumer.ok_or_else(|| {
            RouteOpenError::Fault(format!("{sink:?} audio mirror is not configured"))
        })?;
        (
            super::cpal_audio_output::build_orange_cpal_mirror_stream(
                profile,
                sink,
                consumer,
                health.clone(),
            )?,
            None,
        )
    };
    if let Err(error) = built.play() {
        if let Err(status) = built.teardown() {
            return Err(super::cpal_audio_output::map_shutdown_error(status));
        }
        return Err(super::cpal_audio_output::map_play_stream_error(error));
    }
    let built = if sink == AudioSink::Jack {
        let scheduler = built.scheduler.clone();
        qualify_jack_or_teardown(&scheduler, built, |built| built.teardown())?
    } else {
        qualify_callback_scheduler(
            sink.scheduler_label(),
            &built.scheduler,
            CALLBACK_SCHEDULING_STARTUP_TIMEOUT,
        )
        .map_err(RouteOpenError::Fault)?;
        built
    };
    if let Some(engine_tx) = engine_tx.as_ref() {
        engine_tx
            .send(EngineEvent::SetPreparedInstruments(
                prepare_instruments_config(default_pi_instruments(), DEFAULT_AUDIO_SAMPLE_RATE),
            ))
            .map_err(|error| RouteOpenError::Fault(error.to_string()))?;
    }
    Ok(OpenedAudioSink {
        engine_tx,
        _stream: Some(Box::new(built)),
        health,
        #[cfg(test)]
        _test_engine_rx: None,
    })
}

fn qualify_jack_or_teardown<T>(
    scheduler: &crate::audio_priority::CallbackSchedulingHandle,
    built: T,
    teardown: impl FnOnce(T) -> Result<AudioStreamShutdownReport, AudioStreamShutdownError>,
) -> Result<T, RouteOpenError> {
    if let Err(error) =
        qualify_callback_scheduler("Jack", scheduler, CALLBACK_SCHEDULING_STARTUP_TIMEOUT)
    {
        if let Err(status) = teardown(built) {
            return Err(RouteOpenError::Fault(format!(
                "{error}; audio worker teardown failed: {status:?}"
            )));
        }
        return Err(RouteOpenError::Fault(error));
    }
    Ok(built)
}

pub(super) fn recordings_dir() -> std::path::PathBuf {
    crate::main_paths::default_recordings_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_priority::CallbackSchedulingHandle;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn jack_timeout_tears_down_built_stream_and_workers_before_faulting() {
        let scheduler = CallbackSchedulingHandle::new_jack();
        let stream_dropped = Arc::new(AtomicBool::new(false));
        let workers_joined = Arc::new(AtomicUsize::new(0));
        let dropped = stream_dropped.clone();
        let joined = workers_joined.clone();
        let result = qualify_jack_or_teardown(&scheduler, (), move |_| {
            dropped.store(true, Ordering::Release);
            joined.store(2, Ordering::Release);
            Ok(AudioStreamShutdownReport {
                joined_workers: 2,
                retirement_error: None,
            })
        });

        assert!(
            matches!(result, Err(RouteOpenError::Fault(error)) if error.contains("stage=timeout"))
        );
        assert!(stream_dropped.load(Ordering::Acquire));
        assert_eq!(workers_joined.load(Ordering::Acquire), 2);
    }
}
