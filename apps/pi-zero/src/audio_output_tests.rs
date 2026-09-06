#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use super::cpal_audio_output::resolve_output_buffer_frames;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use platform_core::AUDIO_OUTPUT_BUFFER_FRAMES;

#[cfg(feature = "hardware-orange-pi-zero-2w")]
use crate::audio_replay::ReplayCache;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use crate::audio_sink_registry::{has_sink, register_sink};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use crate::recording::RecorderService;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use rodio_engine_source::{event_queue, EngineEvent, EngineEventReceiver};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use std::time::{Duration, Instant};

#[cfg(feature = "hardware-orange-pi-zero-2w")]
static ORANGE_TAP_OWNER_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "hardware-orange-pi-zero-2w")]
static ORANGE_TAP_ABSENT_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "hardware-orange-pi-zero-2w")]
static ORANGE_LOAD_TX_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "hardware-orange-pi-zero-2w")]
static ORANGE_LOAD_TX_ABSENT_COUNT: AtomicUsize = AtomicUsize::new(0);

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn raspberry_direct_cpal_default_buffer_remains_256_frames() {
    let default_frames = AUDIO_OUTPUT_BUFFER_FRAMES as u32;
    assert_eq!(
        resolve_output_buffer_frames(None, None, default_frames),
        default_frames
    );
    assert_eq!(
        resolve_output_buffer_frames(None, Some(512), default_frames),
        512
    );
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn output_buffer_override_is_parsed_and_clamped() {
    let default_frames = AUDIO_OUTPUT_BUFFER_FRAMES as u32;
    assert_eq!(
        resolve_output_buffer_frames(Some("1024"), None, default_frames),
        1024
    );
    assert_eq!(
        resolve_output_buffer_frames(Some("invalid"), Some(512), default_frames),
        512
    );
    assert_eq!(
        resolve_output_buffer_frames(Some("1"), None, default_frames),
        32
    );
    assert_eq!(
        resolve_output_buffer_frames(Some("4096"), None, default_frames),
        2048
    );
}

#[test]
fn scheduler_labels_preserve_sink_identity() {
    assert_eq!(super::AudioSink::Jack.scheduler_label(), "Jack");
    assert_eq!(super::AudioSink::Usb.scheduler_label(), "USB");
}

#[test]
fn startup_open_classification_is_exhaustive_for_selected_routes() {
    let outputs = playback_runtime::AudioOutputSet::from_flags(true, true, true).unwrap();
    let policy = super::AudioOpenPolicy::Outputs(outputs);
    for error in [
        crate::audio_route::RouteOpenError::Busy,
        crate::audio_route::RouteOpenError::Unsupported("format".into()),
        crate::audio_route::RouteOpenError::Fault("backend".into()),
    ] {
        assert_eq!(
            super::startup_open_action(policy, super::AudioSink::Jack, true, &error),
            super::StartupOpenAction::Fail
        );
        for sink in [super::AudioSink::Usb, super::AudioSink::Hdmi] {
            assert_eq!(
                super::startup_open_action(policy, sink, true, &error),
                super::StartupOpenAction::Ignore
            );
        }
    }
    assert_eq!(
        super::startup_open_action(
            policy,
            super::AudioSink::Jack,
            true,
            &crate::audio_route::RouteOpenError::Absent,
        ),
        super::StartupOpenAction::Fail
    );
    for sink in [super::AudioSink::Usb, super::AudioSink::Hdmi] {
        assert_eq!(
            super::startup_open_action(
                policy,
                sink,
                true,
                &crate::audio_route::RouteOpenError::Absent,
            ),
            super::StartupOpenAction::Wait
        );
        assert_eq!(
            super::startup_open_action(
                policy,
                sink,
                true,
                &crate::audio_route::RouteOpenError::Disconnected,
            ),
            super::StartupOpenAction::Wait
        );
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn raspberry_cpal_errors_preserve_route_classification() {
    assert_eq!(
        super::cpal_audio_output::map_default_config_error(
            cpal::DefaultStreamConfigError::DeviceNotAvailable
        ),
        crate::audio_route::RouteOpenError::Disconnected
    );
    assert_eq!(
        super::cpal_audio_output::map_default_config_error(
            cpal::DefaultStreamConfigError::DeviceBusy
        ),
        crate::audio_route::RouteOpenError::Busy
    );
    assert!(matches!(
        super::cpal_audio_output::map_default_config_error(
            cpal::DefaultStreamConfigError::StreamTypeNotSupported
        ),
        crate::audio_route::RouteOpenError::Unsupported(_)
    ));
    assert_eq!(
        super::cpal_audio_output::map_build_stream_error(cpal::BuildStreamError::DeviceBusy),
        crate::audio_route::RouteOpenError::Busy
    );
    assert_eq!(
        super::cpal_audio_output::map_play_stream_error(cpal::PlayStreamError::DeviceNotAvailable),
        crate::audio_route::RouteOpenError::Disconnected
    );
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn raspberry_audio_manager_rejects_missing_jack_before_device_access() {
    let outputs = playback_runtime::AudioOutputSet::from_flags(false, true, false).unwrap();
    match super::AudioManager::new(None, outputs) {
        Err(error) => assert_eq!(error, "Jack Audio is always on"),
        Ok(_) => panic!("missing Jack should be rejected"),
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_dsp_profiles_select_the_authoritative_audio_geometry() {
    let latency = super::cpal_audio_output::OrangeAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Latency,
    );
    assert_eq!(latency.output_buffer_frames, 128);
    assert_eq!(latency.expected_alsa_period_frames, 32);
    assert_eq!(latency.internal_block_frames, 32);
    assert_eq!(latency.lookahead_frames, 0);

    let capacity = super::cpal_audio_output::OrangeAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Capacity,
    );
    assert_eq!(capacity.output_buffer_frames, 256);
    assert_eq!(capacity.expected_alsa_period_frames, 64);
    assert_eq!(capacity.internal_block_frames, 64);
    assert_eq!(capacity.lookahead_frames, 64);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_audio_manager_rejects_missing_jack_before_device_access() {
    let outputs = playback_runtime::AudioOutputSet::from_flags(false, true, false).unwrap();
    let profile = super::cpal_audio_output::OrangeAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Latency,
    );
    match super::AudioManager::new_orange(profile, outputs) {
        Err(error) => assert_eq!(error.to_string(), "Jack Audio is always on"),
        Ok(_) => panic!("missing Jack should be rejected"),
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_render_quantum_uses_the_capability_default() {
    assert_eq!(
        rodio_engine_source::EngineSource::resolve_block_frames(
            realtime_engine::synth::DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES
        ),
        128
    );
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_scheduler_labels_identify_dac_and_uac2() {
    assert_eq!(super::AudioSink::Jack.scheduler_label(), "Jack");
    assert_eq!(super::AudioSink::Usb.scheduler_label(), "USB");
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_controller_reopens_optional_uac2_once_and_keeps_dac_registered() {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let tap_seen = Arc::new(Mutex::new(Vec::new()));
    let mirror_seen = Arc::new(Mutex::new(Vec::new()));
    let replay_receiver = Arc::new(Mutex::new(None::<Arc<Mutex<EngineEventReceiver>>>));
    let opener: super::orange_audio_recovery::OrangeRecoveryOpener = {
        let attempts = attempts.clone();
        let tap_seen = tap_seen.clone();
        let mirror_seen = mirror_seen.clone();
        let replay_receiver = replay_receiver.clone();
        Arc::new(
            move |_, sink, health, recording_tap, _load_tx, _mirror_producers, mirror_consumer| {
                attempts.lock().unwrap().push(sink);
                tap_seen.lock().unwrap().push(recording_tap.is_some());
                mirror_seen.lock().unwrap().push(mirror_consumer.is_some());
                if attempts.lock().unwrap().len() < 4 {
                    return Err(crate::audio_route::RouteOpenError::Absent);
                }
                let (_tx, rx) = event_queue();
                let rx = Arc::new(Mutex::new(rx));
                *replay_receiver.lock().unwrap() = Some(rx.clone());
                Ok(
                    crate::audio::audio_output::audio_output_open::OpenedAudioSink {
                        engine_tx: None,
                        _stream: None,
                        health,
                        _test_engine_rx: Some(rx),
                    },
                )
            },
        )
    };
    let now = Arc::new(Mutex::new(Instant::now()));
    let clock_now = now.clone();
    let clock: super::orange_audio_recovery::OrangeRecoveryClock =
        Arc::new(move || *clock_now.lock().unwrap());
    let (dac_tx, _dac_rx) = event_queue();
    let sinks = Arc::new(Mutex::new(Vec::new()));
    register_sink(&sinks, super::AudioSink::Jack, dac_tx);
    let mut replay = ReplayCache::default();
    replay.remember(&EngineEvent::SetMasterVolume { volume_pct: 72.0 });
    let replay_events = Arc::new(Mutex::new(replay));
    let mut controller = super::orange_audio_recovery::OrangeRecoveryController::
        new_optional_missing_with_dependencies(
            super::AudioSink::Usb,
            super::cpal_audio_output::OrangeAudioProfile::from_optimization(
                playback_runtime::AudioOptimization::Latency,
            ),
            sinks.clone(),
            replay_events,
            opener,
            clock,
            Some(rodio_engine_source::new_pcm_mirror().producer),
        );

    for _ in 0..3 {
        controller.recover_if_due();
        *now.lock().unwrap() += Duration::from_millis(50);
    }
    *now.lock().unwrap() += Duration::from_secs(2);
    controller.recover_if_due();
    *now.lock().unwrap() += Duration::from_millis(250);
    controller.recover_if_due();

    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        &[super::AudioSink::Usb; 4]
    );
    assert_eq!(tap_seen.lock().unwrap().as_slice(), &[false; 4]);
    assert_eq!(mirror_seen.lock().unwrap().as_slice(), &[true; 4]);
    let registered = sinks.lock().unwrap();
    assert!(registered
        .iter()
        .any(|entry| entry.sink == super::AudioSink::Jack));
    assert!(!registered
        .iter()
        .any(|entry| entry.sink == super::AudioSink::Usb));
    drop(registered);
    let receiver = replay_receiver.lock().unwrap().take().unwrap();
    let mut receiver = receiver.lock().unwrap();
    assert!(receiver.try_recv().is_err());
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_optional_terminal_open_failure_is_attempted_once() {
    let attempts = Arc::new(Mutex::new(0));
    let attempt_counter = attempts.clone();
    let opener: super::orange_audio_recovery::OrangeRecoveryOpener = Arc::new(
        move |_, _, _, _recording_tap, _load_tx, _mirror_producers, _mirror_consumer| {
            *attempt_counter.lock().unwrap() += 1;
            Err(crate::audio_route::RouteOpenError::Busy)
        },
    );
    let now = Arc::new(Mutex::new(Instant::now()));
    let clock_now = now.clone();
    let clock: super::orange_audio_recovery::OrangeRecoveryClock =
        Arc::new(move || *clock_now.lock().unwrap());
    let mut controller = super::orange_audio_recovery::OrangeRecoveryController::
        new_optional_missing_with_dependencies(
            super::AudioSink::Usb,
            super::cpal_audio_output::OrangeAudioProfile::from_optimization(
                playback_runtime::AudioOptimization::Latency,
            ),
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(Mutex::new(ReplayCache::default())),
            opener,
            clock,
            None,
        );

    controller.recover_if_due();
    *now.lock().unwrap() += Duration::from_secs(10);
    controller.recover_if_due();

    assert_eq!(*attempts.lock().unwrap(), 1);
    assert_eq!(controller.device_status(), super::OrangeDacStatus::Terminal);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_required_controller_detaches_after_device_loss_without_opening_hardware() {
    let (tx, _rx) = event_queue();
    let sinks = Arc::new(Mutex::new(Vec::new()));
    register_sink(&sinks, super::AudioSink::Jack, tx.clone());
    let health = crate::audio_stream_health::AudioStreamHealth::new("Jack".into());
    let initial = crate::audio::audio_output::audio_output_open::OpenedAudioSink {
        engine_tx: Some(tx),
        _stream: None,
        health: health.clone(),
        _test_engine_rx: None,
    };
    let replay_events = Arc::new(Mutex::new(ReplayCache::default()));
    let attach_gate = crate::audio_sink_registry::new_attach_gate();
    let mut controller = super::orange_audio_recovery::OrangeRecoveryController::new_required(
        initial,
        super::cpal_audio_output::OrangeAudioProfile::from_optimization(
            playback_runtime::AudioOptimization::Latency,
        ),
        sinks.clone(),
        replay_events,
        None,
        attach_gate,
        [None, None],
    )
    .unwrap();

    health.mark_terminal();
    controller.recover_if_due();

    assert_eq!(controller.device_status(), super::OrangeDacStatus::Terminal);
    assert!(!has_sink(&sinks, super::AudioSink::Jack));
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_multiple_selected_routes_open_one_recording_tap_owner() {
    ORANGE_TAP_OWNER_COUNT.store(0, Ordering::SeqCst);
    ORANGE_TAP_ABSENT_COUNT.store(0, Ordering::SeqCst);
    ORANGE_LOAD_TX_COUNT.store(0, Ordering::SeqCst);
    ORANGE_LOAD_TX_ABSENT_COUNT.store(0, Ordering::SeqCst);

    let outputs = playback_runtime::AudioOutputSet::from_flags(true, true, true).unwrap();
    let manager = super::AudioManager::new_with_opener(
        super::audio_output_open::AudioConstructionConfig::orange(
            super::cpal_audio_output::OrangeAudioProfile::from_optimization(
                playback_runtime::AudioOptimization::Capacity,
            ),
        ),
        super::AudioSink::selected(outputs),
        true,
        super::AudioOpenPolicy::Outputs(outputs),
        orange_test_opener,
        crate::audio_route::new_registry(outputs),
        crate::audio_sink_registry::new_attach_gate(),
    )
    .unwrap();

    assert_eq!(ORANGE_TAP_OWNER_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(ORANGE_TAP_ABSENT_COUNT.load(Ordering::SeqCst), 2);
    assert_eq!(ORANGE_LOAD_TX_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(ORANGE_LOAD_TX_ABSENT_COUNT.load(Ordering::SeqCst), 2);
    drop(manager);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_multiple_routes_record_samples_once_from_the_selected_owner() {
    let outputs = playback_runtime::AudioOutputSet::from_flags(true, true, true).unwrap();
    let owner = crate::audio_recording::recording_owner(outputs).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "octessera-orange-recording-owner-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut recorder = RecorderService::new(directory.clone());
    let tap = recorder.start_audio(1).unwrap();

    for sink in super::AudioSink::selected(outputs) {
        if owner == sink {
            let mut chunk = crate::recording::RecordingChunk::new();
            for sample in [0_i16, i16::MAX, i16::MIN, -1] {
                assert!(chunk.push(sample));
            }
            tap.push_chunk(chunk);
        }
    }
    recorder.stop_audio();

    let path = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().and_then(|extension| extension.to_str()) == Some("wav"))
        .unwrap();
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 8);
    let _ = std::fs::remove_dir_all(directory);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn orange_test_opener(
    _construction: super::audio_output_open::AudioConstructionConfig,
    sink: super::AudioSink,
    recording_tap: Option<super::RecordingTapState>,
    load_tx: Option<rodio_engine_source::AudioLoadStatusSender>,
    mirror_producers: rodio_engine_source::PcmMirrorProducers,
    mirror_consumer: Option<rodio_engine_source::PcmMirrorConsumer>,
) -> Result<super::audio_output_open::OpenedAudioSink, crate::audio_route::RouteOpenError> {
    if sink == super::AudioSink::Jack {
        assert!(mirror_producers.iter().any(Option::is_some));
        assert!(mirror_consumer.is_none());
    } else {
        assert!(mirror_producers.iter().all(Option::is_none));
        assert!(mirror_consumer.is_some());
    }
    if recording_tap.is_some() {
        ORANGE_TAP_OWNER_COUNT.fetch_add(1, Ordering::SeqCst);
    } else {
        ORANGE_TAP_ABSENT_COUNT.fetch_add(1, Ordering::SeqCst);
    }
    if load_tx.is_some() {
        ORANGE_LOAD_TX_COUNT.fetch_add(1, Ordering::SeqCst);
    } else {
        ORANGE_LOAD_TX_ABSENT_COUNT.fetch_add(1, Ordering::SeqCst);
    }
    let (engine_tx, engine_rx) = event_queue();
    Ok(super::audio_output_open::OpenedAudioSink {
        engine_tx: (sink == super::AudioSink::Jack).then_some(engine_tx),
        _stream: None,
        health: crate::audio_stream_health::AudioStreamHealth::optional(format!("{sink:?}")),
        _test_engine_rx: (sink == super::AudioSink::Jack)
            .then_some(Arc::new(Mutex::new(engine_rx))),
    })
}
