use super::audio_output_open::{AudioConstructionConfig, OpenedAudioSink};
use super::cpal_audio_output::OrangeAudioProfile;
use super::{AudioManager, AudioOpenPolicy, AudioSink};
use crate::audio_route::{self, AudioRouteStatus, RouteOpenError};
use crate::audio_sink_registry::new_attach_gate;
use crate::audio_stream_health::AudioStreamHealth;
use playback_runtime::AudioOptimization;
use rodio_engine_source::event_queue;
use std::sync::{Arc, Mutex};

static OPEN_ATTEMPTS: Mutex<Vec<AudioSink>> = Mutex::new(Vec::new());

#[test]
fn orange_optional_terminal_startup_failures_do_not_block_jack() {
    OPEN_ATTEMPTS.lock().unwrap().clear();
    let outputs = playback_runtime::AudioOutputSet::from_flags(true, true, true).unwrap();
    let manager = AudioManager::new_with_opener(
        AudioConstructionConfig::orange(OrangeAudioProfile::from_optimization(
            AudioOptimization::Latency,
        )),
        AudioSink::selected(outputs),
        true,
        AudioOpenPolicy::Outputs(outputs),
        terminal_failure_opener,
        audio_route::new_registry(outputs),
        new_attach_gate(),
    )
    .unwrap();

    assert_eq!(
        *OPEN_ATTEMPTS.lock().unwrap(),
        vec![AudioSink::Jack, AudioSink::Usb, AudioSink::Hdmi]
    );
    assert_eq!(
        audio_route::status(&manager.route_registry, AudioSink::Usb),
        AudioRouteStatus::Faulted
    );
    assert_eq!(
        audio_route::status(&manager.route_registry, AudioSink::Hdmi),
        AudioRouteStatus::Faulted
    );
    assert!(manager.ensure_selected_routes().is_ok());
}

fn terminal_failure_opener(
    _construction: AudioConstructionConfig,
    sink: AudioSink,
    _recording_tap: Option<super::RecordingTapState>,
    _load_tx: Option<rodio_engine_source::AudioLoadStatusSender>,
    _mirror_producers: rodio_engine_source::PcmMirrorProducers,
    _mirror_consumer: Option<rodio_engine_source::PcmMirrorConsumer>,
) -> Result<OpenedAudioSink, RouteOpenError> {
    OPEN_ATTEMPTS.lock().unwrap().push(sink);
    if sink == AudioSink::Usb {
        return Err(RouteOpenError::Busy);
    }
    if sink == AudioSink::Hdmi {
        return Err(RouteOpenError::Unsupported("format".into()));
    }
    let (engine_tx, engine_rx) = event_queue();
    Ok(OpenedAudioSink {
        engine_tx: Some(engine_tx),
        _stream: None,
        health: AudioStreamHealth::new("Jack".into()),
        _test_engine_rx: Some(Arc::new(Mutex::new(engine_rx))),
    })
}
