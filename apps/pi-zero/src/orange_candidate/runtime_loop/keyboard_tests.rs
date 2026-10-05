use crate::host_adapter::PiHostAdapter;
use crate::runtime_output::ingest_oled_messages;
use playback_runtime::RunnerMessage;
use serde_json::json;

#[test]
fn accepted_snapshot_ingestion_updates_orange_keyboard_gate() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-orange-keyboard-snapshot");
    let (audio, _, _, _) = crate::audio::test_service_with_prep_sender();
    let mut host = PiHostAdapter::with_directories(
        audio,
        root.join("store"),
        root.join("samples"),
        std::sync::Arc::new(|_| {}),
        false,
    )
    .unwrap();
    let control = crate::keyboard_capture::KeyboardCaptureControl::new(true);
    host.core.set_keyboard_capture_control(control.clone());
    ingest_oled_messages(
        &mut host,
        &[RunnerMessage::Snapshot {
            snapshot: json!({"hdmi":{"mode":"live-grid"}}),
        }],
    );
    assert!(control.is_enabled());
    ingest_oled_messages(
        &mut host,
        &[RunnerMessage::Snapshot {
            snapshot: json!({"hdmi":{"mode":"none"}}),
        }],
    );
    assert!(!control.is_enabled());
    let _ = std::fs::remove_dir_all(root);
}
