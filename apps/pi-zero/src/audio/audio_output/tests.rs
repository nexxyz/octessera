#[cfg(feature = "hardware-orange-pi-zero-2w")]
use crate::audio_engine_owner::AudioEngineOwner;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use crate::audio_replay::ReplayCache;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use media_recording::RecorderService;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use rodio_engine_source::event_queue;

#[test]
fn scheduler_labels_preserve_sink_identity() {
    assert_eq!(super::AudioSink::Jack.scheduler_label(), "Jack");
    assert_eq!(super::AudioSink::Usb.scheduler_label(), "USB");
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
    match super::AudioManager::new(playback_runtime::AudioOptimization::Latency, outputs) {
        Err(error) => assert_eq!(error, "Jack Audio is always on"),
        Ok(_) => panic!("missing Jack should be rejected"),
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn raspberry_dsp_profiles_select_the_authoritative_audio_geometry() {
    let latency = super::audio_profile::RaspberryAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Latency,
    );
    assert_eq!(latency.output_buffer_frames, 256);
    assert_eq!(latency.expected_alsa_period_frames, 64);
    assert_eq!(latency.internal_block_frames, 128);
    assert_eq!(latency.lookahead_frames, 0);

    let capacity = super::audio_profile::RaspberryAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Capacity,
    );
    assert_eq!(capacity.output_buffer_frames, 256);
    assert_eq!(capacity.expected_alsa_period_frames, 64);
    assert_eq!(capacity.internal_block_frames, 256);
    assert_eq!(capacity.lookahead_frames, 256);
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn raspberry_timing_probe_profile_applies_only_requested_geometry_overrides() {
    let profile =
        super::audio_profile::RaspberryAudioProfile::from_timing_probe(Some(512), Some(64));
    assert_eq!(profile.output_buffer_frames, 512);
    assert_eq!(profile.internal_block_frames, 64);
    assert_eq!(profile.lookahead_frames, 0);

    let clamped =
        super::audio_profile::RaspberryAudioProfile::from_timing_probe(Some(1), Some(4096));
    assert_eq!(clamped.output_buffer_frames, 32);
    assert_eq!(clamped.internal_block_frames, 2_048);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_dsp_profiles_select_the_authoritative_audio_geometry() {
    let latency = super::audio_profile::OrangeAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Latency,
    );
    assert_eq!(latency.output_buffer_frames, 128);
    assert_eq!(latency.expected_alsa_period_frames, 32);
    assert_eq!(latency.internal_block_frames, 32);
    assert_eq!(latency.lookahead_frames, 0);

    let capacity = super::audio_profile::OrangeAudioProfile::from_optimization(
        playback_runtime::AudioOptimization::Capacity,
    );
    assert_eq!(capacity.output_buffer_frames, 256);
    assert_eq!(capacity.expected_alsa_period_frames, 64);
    assert_eq!(capacity.internal_block_frames, 128);
    assert_eq!(capacity.lookahead_frames, 128);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_audio_manager_rejects_missing_jack_before_device_access() {
    let outputs = playback_runtime::AudioOutputSet::from_flags(false, true, false).unwrap();
    let profile = super::audio_profile::OrangeAudioProfile::from_optimization(
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
fn orange_required_controller_detaches_after_device_loss_without_opening_hardware() {
    let (tx, _rx) = event_queue();
    let engine = AudioEngineOwner::new(ReplayCache::default());
    let health = crate::audio_stream_health::AudioStreamHealth::new("Jack".into());
    let initial = crate::audio::audio_output::audio_output_open::OpenedAudioSink {
        engine_tx: Some(tx),
        _stream: None,
        health: health.clone(),
        _test_engine_rx: None,
    };
    let mut controller = super::orange_audio_recovery::OrangeRecoveryController::new_required(
        initial,
        super::audio_profile::OrangeAudioProfile::from_optimization(
            playback_runtime::AudioOptimization::Latency,
        ),
        engine.clone(),
        None,
        [None, None],
    )
    .unwrap();

    health.mark_terminal();
    controller.recover_if_due();

    assert_eq!(controller.device_status(), super::OrangeDacStatus::Terminal);
    assert!(!engine.is_attached());
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_multiple_routes_record_samples_once_from_the_selected_owner() {
    let outputs = playback_runtime::AudioOutputSet::from_flags(true, true, true).unwrap();
    let owner = crate::audio_recording::recording_owner(outputs).unwrap();
    let directory = crate::test_temp_dir::unique_temp_path("octessera-orange-recording-owner");
    let mut recorder = RecorderService::new(directory.clone());
    let tap = recorder.start_audio(1).unwrap();

    for sink in super::AudioSink::selected(outputs) {
        if owner == sink {
            let mut chunk = tap.new_chunk();
            assert!(chunk.push_frame(0, i16::MAX));
            assert!(chunk.push_frame(i16::MIN, -1));
            tap.push_chunk(chunk);
        }
    }
    let outcome = recorder.stop_audio().unwrap().unwrap();
    assert_eq!(outcome.frames_written, 2);

    let path = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().and_then(|extension| extension.to_str()) == Some("wav"))
        .unwrap();
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 8);
    let _ = std::fs::remove_dir_all(directory);
}
