use super::{platform_request, temp_store_dir, test_adapter_with_recording_dir};
use media_recording::RecordingStatus;
use playback_runtime::{
    HostAdapter, HostMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimePlatformEffect,
    RuntimeStoreResult,
};
use std::fs;

#[test]
fn desktop_recording_start_and_stop_finalize_a_valid_wav() {
    let directory = temp_store_dir("recording-valid");
    let (mut adapter, _) = test_adapter_with_recording_dir(directory.clone());

    assert!(matches!(
        adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::RecordingStartAudio { max_minutes: 1 }
        ))
        .unwrap()
        .as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RecordingStatus {
                ok: true,
                message,
                active: true,
            }
        }] if message == "Recording started"
    ));
    assert!(matches!(
        adapter
            .handle_platform_effect(&platform_request(RuntimePlatformEffect::RecordingStop))
            .unwrap()
            .as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RecordingStatus {
                ok: true,
                message,
                active: false,
            }
        }] if message == "Recording saved"
    ));
    assert!(adapter.poll_recording_status().is_none());

    let path = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "wav"))
        .expect("final WAV");
    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 2);
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        44_100
    );
    assert_eq!(u16::from_le_bytes(bytes[34..36].try_into().unwrap()), 16);
    assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 0);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn desktop_audio_oled_effect_uses_screen_recording_mode_and_rejects_audio_start() {
    let directory = temp_store_dir("recording-oled");
    let (mut adapter, _) = test_adapter_with_recording_dir(directory.clone());

    let started = adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::RecordingStartAudioOled { max_minutes: 1 },
        ))
        .unwrap();
    assert!(matches!(
        started.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RecordingStatus {
                ok: true,
                message,
                active: true,
            }
        }] if message == "Recording started"
    ));
    let active = adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::RecordingStartAudio { max_minutes: 1 },
        ))
        .unwrap();
    assert!(matches!(
        active.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RecordingStatus {
                ok: true,
                message,
                active: true,
            }
        }] if message == "Recording is already running"
    ));
    adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::RecordingStop))
        .unwrap();

    assert!(fs::read_dir(&directory).unwrap().any(|entry| entry
        .unwrap()
        .path()
        .extension()
        .is_some_and(|extension| extension == "avi")));
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn desktop_recording_reports_already_active_and_filesystem_errors() {
    let directory = temp_store_dir("recording-active");
    let (mut adapter, _) = test_adapter_with_recording_dir(directory.clone());
    adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::RecordingStartAudio { max_minutes: 1 },
        ))
        .unwrap();
    let active = adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::RecordingStartAudio { max_minutes: 1 },
        ))
        .unwrap();
    assert!(matches!(
        active.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RecordingStatus {
                ok: true,
                message,
                active: true,
            }
        }] if message == "Recording is already running"
    ));
    adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::RecordingStop))
        .unwrap();
    let _ = fs::remove_dir_all(directory);

    let file_path = temp_store_dir("recording-file").join("not-a-directory");
    fs::write(&file_path, []).unwrap();
    let (mut adapter, _) = test_adapter_with_recording_dir(file_path.clone());
    let error = adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::RecordingStartAudio { max_minutes: 1 },
        ))
        .unwrap_err();
    assert_eq!(error.facts.domain, RuntimeErrorDomain::Recording);
    assert_eq!(error.facts.code, RuntimeErrorCode::OperationFailed);
    assert!(error
        .facts
        .message
        .as_deref()
        .is_some_and(|message| message.contains("recording directory unavailable")));
    let _ = fs::remove_file(file_path.clone());
    let _ = fs::remove_dir_all(file_path.parent().unwrap());

    let directory = temp_store_dir("recording-stop-error");
    let (mut adapter, _) = test_adapter_with_recording_dir(directory.clone());
    adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::RecordingStartAudio { max_minutes: 1 },
        ))
        .unwrap();
    let partial = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.to_string_lossy().ends_with(".partial.wav"))
        .expect("partial WAV");
    let stem = partial
        .file_name()
        .unwrap()
        .to_string_lossy()
        .trim_end_matches(".partial.wav")
        .to_string();
    fs::create_dir(directory.join(format!("{stem}.wav"))).unwrap();
    let error = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::RecordingStop))
        .unwrap_err();
    assert_eq!(error.facts.domain, RuntimeErrorDomain::Recording);
    assert_eq!(error.facts.code, RuntimeErrorCode::OperationFailed);
    assert!(error
        .facts
        .message
        .as_deref()
        .is_some_and(|message| message.contains("already exists")));
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn desktop_recording_stop_when_idle_is_harmless() {
    let directory = temp_store_dir("recording-idle");
    let (mut adapter, _) = test_adapter_with_recording_dir(directory.clone());
    assert!(adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::RecordingStop))
        .unwrap()
        .is_empty());
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn desktop_recording_completion_uses_existing_status_toast_result() {
    let saved =
        super::super::host_adapter_recording::status_for_recording(RecordingStatus::Complete);
    assert!(matches!(
        saved,
        RuntimeStoreResult::RecordingStatus {
            ok: true,
            message,
            active: false,
        }
            if message == "Recording saved"
    ));

    let incomplete =
        super::super::host_adapter_recording::status_for_recording(RecordingStatus::Incomplete {
            gap_count: 0,
            gap_frames: 0,
            overflow_count: 0,
            overflow_frames: 2,
        });
    assert!(matches!(
        incomplete,
        RuntimeStoreResult::RecordingStatus {
            ok: false,
            message,
            active: false,
        }
            if message == "Recording incomplete"
    ));

    let max_time = super::super::host_adapter_recording::max_time_status_for_recording(
        RecordingStatus::Complete,
    );
    assert!(matches!(
        max_time,
        RuntimeStoreResult::RecordingStatus {
            ok: true,
            message,
            active: false,
        } if message == "Max time: saved"
    ));
}
