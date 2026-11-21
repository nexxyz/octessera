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

#[test]
fn stutter_release_fades_back_to_the_dry_sound() {
    let segment_len = 6_000;
    let mut voice = StutterVoice::new(
        StutterParams {
            depth: 1.0,
            segment_len,
        },
        48_000,
    );
    for _ in 0..segment_len + 96 {
        voice.process(1.0, 1.0);
    }
    for _ in 0..1_000 {
        voice.process(0.0, 0.0);
    }
    voice.release();
    let mut released = Vec::new();
    while !voice.finished() {
        assert!(released.len() < 480, "the release never ended");
        released.push(voice.process(0.0, 0.0).0);
    }
    assert!((235..=241).contains(&released.len()), "{}", released.len());
    let largest_step = released
        .windows(2)
        .map(|pair| (pair[0] - pair[1]).abs())
        .fold(0.0, f32::max);
    assert!(largest_step < 0.01, "step {largest_step}");
    assert!(released.last().unwrap().abs() < 0.01);
}

#[test]
fn stutter_released_before_its_capture_ends_at_once() {
    let mut voice = StutterVoice::new(
        StutterParams {
            depth: 1.0,
            segment_len: 6_000,
        },
        48_000,
    );
    voice.process(0.3, 0.3);
    voice.release();
    assert!(voice.finished());
}
