use super::*;
use crate::{OledIngressState, RecordingStatus};
use std::fs::{self, OpenOptions};

#[test]
fn future_frames_stay_bounded_and_screen_loss_keeps_the_take_complete() {
    let dir =
        std::env::temp_dir().join(format!("octessera-avi-timeline-cap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let partial = dir.join("take.partial.avi");
    let final_path = dir.join("take.avi");
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .unwrap();
    let oled = OledIngress {
        state: Arc::new(OledIngressState::new()),
    };
    let writer = AviWriter::new(
        file,
        partial,
        final_path,
        super::super::AVI_TARGET_MAX_BYTES,
    )
    .unwrap();
    let mut timeline = Timeline::new(writer, oled.clone());

    for revision in 0..TIMELINE_PENDING_CAP as u64 {
        assert!(
            oled.try_submit(future_frame(revision, 10_000 + revision, revision as u8))
                .accepted
        );
    }
    timeline.video_bytes(0).unwrap();
    assert_eq!(timeline.pending_frames.len(), TIMELINE_PENDING_CAP);
    assert_eq!(timeline.material_loss, 0);

    for revision in TIMELINE_PENDING_CAP as u64..(TIMELINE_PENDING_CAP * 2) as u64 {
        assert!(
            oled.try_submit(future_frame(revision, 20_000 + revision, revision as u8))
                .accepted
        );
    }
    timeline.video_bytes(0).unwrap();
    assert_eq!(timeline.pending_frames.len(), TIMELINE_PENDING_CAP);
    assert!(timeline.material_loss > 0);

    assert!(!timeline.push_samples(&vec![0_i16; VIDEO_UNIT_FRAMES * 2]));
    let frames = timeline.audio_frames();
    let outcome = timeline
        .finish(
            FinishStats {
                frames,
                gap_count: 0,
                gap_frames: 0,
                overflow_count: 0,
                overflow_frames: 0,
                incomplete: false,
            },
            0,
        )
        .unwrap();
    assert!(!outcome.path.to_string_lossy().contains(".incomplete."));
    assert_eq!(outcome.status, RecordingStatus::Complete);
    let bytes = fs::read(outcome.path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
        bytes.len() - 8
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn led_changes_appear_on_their_audio_tick_and_repeats_are_skipped() {
    let dir = std::env::temp_dir().join(format!("octessera-avi-leds-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let partial = dir.join("take.partial.avi");
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .unwrap();
    let oled = OledIngress {
        state: Arc::new(OledIngressState::new()),
    };
    let writer = AviWriter::new(
        file,
        partial,
        dir.join("take.avi"),
        super::super::AVI_TARGET_MAX_BYTES,
    )
    .unwrap();
    let mut timeline = Timeline::new(writer, oled.clone());
    let mut lit = LedColors::DARK;
    lit.grid[0] = [255, 0, 0];

    assert!(oled.submit_leds(LedFrame::new(VIDEO_UNIT_FRAMES as u64, lit)));
    assert!(!oled.submit_leds(LedFrame::new(VIDEO_UNIT_FRAMES as u64 + 1, lit)));

    let red = |frame: Vec<u8>| {
        image::load_from_memory(&frame)
            .unwrap()
            .to_rgb8()
            .get_pixel(152, 8)
            .0[0]
    };
    assert_eq!(red(timeline.video_bytes(0).unwrap()), 0);
    assert_eq!(
        red(timeline.video_bytes(VIDEO_UNIT_FRAMES as u64).unwrap()),
        255
    );
    assert!(timeline.pending_leds.is_empty());
    drop(timeline);
    let _ = fs::remove_dir_all(dir);
}

fn future_frame(revision: u64, audio_frame: u64, value: u8) -> OledFrame {
    let mut pixels = [0_u8; OLED_FRAME_BYTES];
    pixels[0] = value;
    pixels[1] = 0xff;
    OledFrame::new(revision, audio_frame, pixels)
}
