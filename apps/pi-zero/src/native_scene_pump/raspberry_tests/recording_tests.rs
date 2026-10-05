use super::{playing_runner, worker_with_recording};
use crate::hardware_runtime_scheduler::SNAPSHOT_TICK;
use crate::host_adapter::PiHostAdapter;
use playback_runtime::{
    HostAdapter, RunnerMessage, RuntimePlatformEffect, RuntimePlatformRequest, UsbDataRole,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn recording_start_seeds_last_physical_typed_frame_not_stale_snapshot_cache() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-physical-oled-recording");
    let actual_root = root.join("actual");
    let startup_reference_root = root.join("startup-reference");
    let typed_reference_root = root.join("typed-reference");
    let mut event_adapter = super::adapter(&root, None);
    let (playback, mut runner) = playing_runner(&mut event_adapter);
    let (audio, _, _, _) = crate::audio::test_service_with_recording_dir(actual_root.clone());
    let mut adapter = PiHostAdapter::new_with_data_role(
        Some(audio.clone()),
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
        UsbDataRole::Gadget,
    );
    let legacy_pixels = vec![0x11; playback_runtime::oled_frame::OLED_FRAME_BYTES];
    adapter.core.ingest_oled_frame(&RunnerMessage::OledFrame {
        revision: 1,
        width: 128,
        height: 128,
        format: "rgb565be".into(),
        pixels: legacy_pixels.clone(),
    });
    let legacy_snapshot = serde_json::json!({"oledFrameRevision": 1});
    adapter.core.accept_oled_frame_reference(&legacy_snapshot);
    assert_eq!(
        adapter
            .core
            .oled_frame_cache
            .accepted_frame()
            .unwrap()
            .pixels(),
        legacy_pixels
    );

    let worker = worker_with_recording(Some(audio.clone()));
    let (startup_revision, startup_pixels) = audio.latest_physical_oled_frame().unwrap();
    assert!(startup_revision > 1);
    assert_eq!(
        startup_pixels.as_ref(),
        vec![0; playback_runtime::oled_frame::OLED_FRAME_BYTES]
    );
    let _ = start_oled_recording(&mut adapter, "startup-physical-oled");
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let actual_startup = audio.stop_recording_with_outcome().unwrap().unwrap();
    let (startup_reference, _, _, _) =
        crate::audio::test_service_with_recording_dir(startup_reference_root);
    startup_reference
        .start_recording_audio_oled_with_seed(1, Some((startup_revision, startup_pixels.to_vec())))
        .unwrap();
    startup_reference
        .test_push_recording_samples(&vec![0; 8_820])
        .unwrap();
    let expected_startup = startup_reference
        .stop_recording_with_outcome()
        .unwrap()
        .unwrap();
    assert_eq!(
        avi_video_payloads(&actual_startup.path)[0],
        avi_video_payloads(&expected_startup.path)[0]
    );

    assert!(runner.pending_display_scene_generation().is_some());
    let pump_started = Instant::now();
    let mut pump = super::super::NativeScenePump::new(pump_started - SNAPSHOT_TICK);
    assert!(pump
        .submit(
            pump_started,
            crate::hardware_runtime_scheduler::DisplaySnapshotDue::default(),
            &playback,
            &mut runner,
            &mut adapter,
            &worker,
        )
        .is_some());
    let deadline = Instant::now() + Duration::from_secs(2);
    while runner.display_scene_pending() && Instant::now() < deadline {
        pump.poll(&mut runner);
        std::thread::sleep(Duration::from_millis(1));
    }
    pump.poll(&mut runner);
    assert!(!runner.display_scene_pending());

    let (revision, pixels) = audio.latest_physical_oled_frame().unwrap();
    assert!(revision > 1);
    assert_ne!(pixels.as_ref(), legacy_pixels);
    let _started = start_oled_recording(&mut adapter, "physical-oled-recording-start");
    audio.test_push_recording_samples(&vec![0; 8_820]).unwrap();
    let actual = audio.stop_recording_with_outcome().unwrap().unwrap();

    let (reference, _, _, _) = crate::audio::test_service_with_recording_dir(typed_reference_root);
    reference
        .start_recording_audio_oled_with_seed(1, Some((revision, pixels.to_vec())))
        .unwrap();
    reference
        .test_push_recording_samples(&vec![0; 8_820])
        .unwrap();
    let expected = reference.stop_recording_with_outcome().unwrap().unwrap();

    assert_eq!(
        avi_video_payloads(&actual.path)[0],
        avi_video_payloads(&expected.path)[0]
    );
    worker.publish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

fn start_oled_recording(
    adapter: &mut PiHostAdapter,
    id: &str,
) -> Vec<playback_runtime::HostMessage> {
    let response = adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::RecordingStartAudioOled { max_minutes: 1 },
            id.into(),
            None,
        ))
        .unwrap();
    assert!(matches!(
        response.as_slice(),
        [playback_runtime::HostMessage::RuntimeResult {
            result: playback_runtime::RuntimeStoreResult::RecordingStatus { ok: true, .. }
        }]
    ));
    response
}

fn avi_video_payloads(path: &std::path::Path) -> Vec<Vec<u8>> {
    let bytes = std::fs::read(path).unwrap();
    let list = bytes
        .windows(12)
        .position(|window| &window[..4] == b"LIST" && &window[8..12] == b"movi")
        .unwrap();
    let mut offset = list + 12;
    let end = list + 8 + u32::from_le_bytes(bytes[list + 4..list + 8].try_into().unwrap()) as usize;
    let mut videos = Vec::new();
    while offset + 8 <= end {
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let payload_end = offset + 8 + size;
        if &bytes[offset..offset + 4] == b"00dc" {
            videos.push(bytes[offset + 8..payload_end].to_vec());
        }
        offset = payload_end + size % 2;
    }
    videos
}
