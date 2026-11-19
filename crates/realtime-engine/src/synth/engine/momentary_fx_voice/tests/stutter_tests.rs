use super::super::{StutterParams, StutterVoice};
use std::f32::consts::TAU;

#[test]
fn stutter_repeats_join_without_a_seam_jump() {
    let sample_rate = 48_000;
    let segment_len = 6_000;
    let ramp_len = 96;
    let period = segment_len / 3;
    let wave = |frame: usize| (TAU * (frame % period) as f32 / period as f32).sin();
    let mut voice = StutterVoice::new(
        StutterParams {
            depth: 1.0,
            segment_len,
        },
        sample_rate,
    );
    for frame in 0..segment_len + ramp_len {
        voice.process(wave(frame), wave(frame));
    }
    for frame in 0..segment_len * 3 {
        let (left, right) = voice.process(0.0, 0.0);
        if frame >= ramp_len {
            assert!((left - wave(frame)).abs() < 1e-4, "left frame {frame}");
            assert!((right - wave(frame)).abs() < 1e-4, "right frame {frame}");
        }
    }
}

#[test]
fn stutter_repeats_once_per_segment() {
    let segment_len = 6_000;
    let mut voice = StutterVoice::new(
        StutterParams {
            depth: 1.0,
            segment_len,
        },
        48_000,
    );
    let output: Vec<f32> = (0..segment_len * 4)
        .map(|frame| voice.process(frame as f32, frame as f32).0)
        .collect();
    let repeats = &output[segment_len * 2..];
    assert_ne!(repeats[0], repeats[1]);
    for frame in 0..segment_len {
        assert_eq!(
            repeats[frame],
            repeats[frame + segment_len],
            "frame {frame}"
        );
    }
}

#[test]
fn stutter_handles_segments_shorter_than_its_ramp() {
    for segment_len in 1..8 {
        let mut voice = StutterVoice::new(
            StutterParams {
                depth: 1.0,
                segment_len,
            },
            48_000,
        );
        for frame in 0..64 {
            let (left, right) = voice.process(frame as f32, -(frame as f32));
            assert!(left.is_finite() && right.is_finite());
        }
    }
}
