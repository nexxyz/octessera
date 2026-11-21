use super::*;
use std::fs::{self, File};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "octessera-media-header-failure-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

/// A handle that refuses writes stands in for a card with no room left for the header.
fn unwritable(partial: &std::path::Path, reserved: File) -> File {
    drop(reserved);
    File::open(partial).unwrap()
}

#[test]
fn a_wav_take_whose_header_cannot_be_written_leaves_no_file() {
    let dir = temp_dir("wav");
    let (partial, final_path, reserved) =
        crate::service::reserve_paths_for_test(&dir, "full").unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let error = crate::wav::write_recording(
        unwritable(&partial, reserved),
        partial.clone(),
        final_path,
        rx,
        Arc::new(AtomicBool::new(true)),
        Arc::new(IngressState::new()),
        8,
    )
    .unwrap_err();
    assert!(error.to_string().contains("audio recording failed"));
    assert!(!partial.exists());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn an_avi_take_whose_header_cannot_be_written_leaves_no_file() {
    let dir = temp_dir("avi");
    let (partial, final_path, reserved) =
        crate::service::reserve_avi_paths_for_test(&dir, "full").unwrap();
    let (_tx, rx) = mpsc::sync_channel(1);
    let result = crate::avi::write_recording(crate::avi::AviRecordingInput {
        file: unwritable(&partial, reserved),
        partial_path: partial.clone(),
        final_path,
        rx,
        stop: Arc::new(AtomicBool::new(true)),
        state: Arc::new(IngressState::new()),
        oled: OledIngress {
            state: Arc::new(OledIngressState::new()),
        },
        max_frames: 8,
        file_limit: AVI_TARGET_MAX_BYTES,
    });
    assert!(result.is_err());
    assert!(!partial.exists());
    let _ = fs::remove_dir_all(dir);
}
