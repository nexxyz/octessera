use super::*;
use crate::audio::{test_service, test_service_with_outputs, test_service_with_recording_dir};
use playback_runtime::{AudioOutputSet, RuntimeErrorCode};

fn assert_sd2_start_rejected(response: &[HostMessage], message: &str) {
    let [HostMessage::RuntimeResult {
        result: RuntimeStoreResult::RuntimeFailure { error },
    }] = response
    else {
        panic!("expected one SD2 gate failure");
    };
    assert_eq!(error.message.as_deref(), Some(message));
}

#[test]
fn orange_sd2_start_rejects_active_usb_audio() {
    let (store, samples) = directories();
    let audio = test_service_with_outputs(AudioOutputSet::from_flags(false, true, false).unwrap());
    let mut adapter = OrangeHostAdapter::with_directories(
        audio,
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();

    let response = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::UsbSdTransferStart,
            "sd2-usb-audio",
        ))
        .unwrap();
    assert_sd2_start_rejected(
        &response,
        "USB SD2 transfer blocked while USB audio out is active",
    );
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn orange_sd2_start_rejects_enabled_usb_midi() {
    let (audio, _, _) = test_service();
    let (store, samples) = directories();
    let mut adapter = OrangeHostAdapter::with_directories(
        audio,
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        true,
    )
    .unwrap();

    let response = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::UsbSdTransferStart,
            "sd2-usb-midi",
        ))
        .unwrap();
    assert_sd2_start_rejected(
        &response,
        "USB SD2 transfer blocked while USB MIDI out is enabled",
    );
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn orange_sd2_start_rejects_active_recording() {
    let (store, samples) = directories();
    let recordings = store.parent().unwrap().join("recordings");
    let (audio, _, _, _) = test_service_with_recording_dir(recordings);
    audio.start_recording(1).unwrap();
    let mut adapter = OrangeHostAdapter::with_directories(
        audio.clone(),
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();

    let response = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::UsbSdTransferStart,
            "sd2-recording",
        ))
        .unwrap();
    assert_sd2_start_rejected(
        &response,
        "USB SD2 transfer blocked while recording is active",
    );
    assert!(audio.is_recording().unwrap());
    audio.stop_recording().unwrap();
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn orange_recording_effect_writes_internal_stereo_wav_and_stops_cleanly() {
    let (store, samples) = directories();
    let recordings = store.parent().unwrap().join("recordings");
    let (audio, _, _, _) = test_service_with_recording_dir(recordings.clone());
    let mut adapter = OrangeHostAdapter::with_directories(
        audio.clone(),
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();

    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::RecordingStartAudio {
                max_minutes: u16::MAX,
            },
            "recording-start",
        ))
        .unwrap()
        .is_empty());
    assert!(audio.is_recording().unwrap());
    audio
        .test_push_recording_samples(&[0, i16::MAX, i16::MIN, -1])
        .unwrap();
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::RecordingStop,
            "recording-stop",
        ))
        .unwrap()
        .is_empty());
    assert!(!audio.is_recording().unwrap());

    let path = std::fs::read_dir(&recordings)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().and_then(|extension| extension.to_str()) == Some("wav"))
        .expect("final Orange recording");
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 2);
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        44_100
    );
    assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 16);
    assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 8);
    assert_eq!(&bytes[44..], &[0, 0, 0xff, 0x7f, 0, 0x80, 0xff, 0xff]);

    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn orange_recording_directory_failure_is_typed_and_does_not_stop_runtime() {
    let (store, samples) = directories();
    let recordings = store.parent().unwrap().join("recordings-file");
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    std::fs::write(&recordings, b"not a directory").unwrap();
    let (audio, _, _, _) = test_service_with_recording_dir(recordings);
    let mut adapter = OrangeHostAdapter::with_directories(
        audio.clone(),
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();

    let error = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::RecordingStartAudio { max_minutes: 1 },
            "recording-failure",
        ))
        .unwrap_err();
    assert_eq!(error.facts.code, RuntimeErrorCode::OperationFailed);
    assert!(!audio.is_recording().unwrap());
    assert!(!adapter.shutdown_pending());

    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}

#[test]
fn orange_power_save_stops_recording_before_power_submission() {
    let (store, samples) = directories();
    let recordings = store.parent().unwrap().join("recordings");
    let (audio, _, _, _) = test_service_with_recording_dir(recordings);
    let mut adapter = OrangeHostAdapter::with_directories(
        audio.clone(),
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    audio.start_recording(1).unwrap();
    adapter.recovery_save_status = Some(Ok(()));

    assert_eq!(adapter.save_recovery_for_power(), Ok(()));
    assert!(!audio.is_recording().unwrap());

    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}
