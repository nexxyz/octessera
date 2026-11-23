use super::{Hermite, OnceThrough, SampleCursor, SampleInterpolation, WholeFrames};
use crate::synth::engine::sample_voice::SampleBufferView;
use crate::synth::SampleBuffer;
use std::hint::black_box;
use std::time::Instant;

fn buffer(samples: Vec<f32>) -> SampleBuffer {
    SampleBuffer {
        samples: samples.into(),
        channels: 1,
        sample_rate: 44_100,
    }
}

struct Linear;

impl SampleInterpolation for Linear {
    #[inline(always)]
    fn at(source: &SampleBufferView, pos: f64) -> f32 {
        let frame = pos as usize;
        let frac = (pos - frame as f64) as f32;
        let next_frame = (frame + 1).min(source.frames() - 1);
        source.mono_frame(frame) * (1.0 - frac) + source.mono_frame(next_frame) * frac
    }
}

#[test]
fn hermite_passes_exactly_through_every_frame() {
    let samples: Vec<f32> = (0..97)
        .map(|frame| ((frame * 37 % 23) as f32 - 11.0) / 13.0)
        .collect();
    let buffer = buffer(samples.clone());
    let view = SampleBufferView::from_buffer(&buffer);
    for (frame, sample) in samples.iter().enumerate() {
        assert_eq!(Hermite::at(&view, frame as f64).to_bits(), sample.to_bits());
        assert_eq!(
            WholeFrames::at(&view, frame as f64).to_bits(),
            sample.to_bits()
        );
    }
}

#[test]
fn hermite_follows_a_quadratic_between_frames() {
    let curve = |x: f64| (0.001 * x * x - 0.02 * x + 0.1) as f32;
    let buffer = buffer((0..64).map(|frame| curve(frame as f64)).collect());
    let view = SampleBufferView::from_buffer(&buffer);
    for frame in 1..61 {
        for fraction in [0.25, 0.5, 0.75] {
            let pos = frame as f64 + fraction;
            assert!(
                (Hermite::at(&view, pos) - curve(pos)).abs() < 1.0e-5,
                "{pos}"
            );
        }
    }
}

#[test]
fn hermite_tracks_a_sine_far_closer_than_linear() {
    let wave = |x: f64| (x * std::f64::consts::TAU / 8.0).sin() as f32;
    let buffer = buffer((0..256).map(|frame| wave(frame as f64)).collect());
    let view = SampleBufferView::from_buffer(&buffer);
    let mut hermite_error = 0.0_f32;
    let mut linear_error = 0.0_f32;
    for step in 8..1_000 {
        let pos = step as f64 * 0.247;
        hermite_error = hermite_error.max((Hermite::at(&view, pos) - wave(pos)).abs());
        linear_error = linear_error.max((Linear::at(&view, pos) - wave(pos)).abs());
    }
    assert!(
        hermite_error < linear_error / 3.0,
        "{hermite_error} vs {linear_error}"
    );
}

#[test]
fn hermite_clamps_its_neighbours_at_the_sample_edges() {
    let buffer = buffer(vec![0.0, 1.0]);
    let view = SampleBufferView::from_buffer(&buffer);
    assert_eq!(Hermite::at(&view, 0.5), 0.5);
    assert_eq!(Hermite::at(&view, 1.5), 1.0625);
    let single = self::buffer(vec![0.25]);
    let view = SampleBufferView::from_buffer(&single);
    assert_eq!(Hermite::at(&view, 0.5), 0.25);
}

fn read_cost<I: SampleInterpolation>(view: &SampleBufferView, step: f64) -> f64 {
    const READS: usize = 4_000_000;
    let mut pos = 0.0;
    let mut sum = 0.0_f32;
    let started = Instant::now();
    for _ in 0..READS {
        if pos >= view.end_position() - 4.0 {
            pos = 0.0;
        }
        let mut cursor = OnceThrough;
        sum += cursor
            .read::<I>(black_box(view), &mut pos, black_box(step))
            .unwrap_or(0.0);
    }
    black_box(sum);
    started.elapsed().as_nanos() as f64 / READS as f64
}

/// Prints what one interpolated sample read costs; run with
/// `cargo test --release -p realtime-engine --lib sample_interpolation_cost -- --ignored --nocapture`.
#[test]
#[ignore = "release-mode micro-benchmark"]
fn sample_interpolation_cost() {
    let buffer = buffer(
        (0..1 << 16)
            .map(|frame| (frame as f32 * 0.013).sin())
            .collect(),
    );
    let view = SampleBufferView::from_buffer(&buffer);
    for _ in 0..2 {
        println!(
            "sample read: linear {:.2} ns, hermite {:.2} ns, whole frames {:.2} ns per sample",
            read_cost::<Linear>(&view, 1.37),
            read_cost::<Hermite>(&view, 1.37),
            read_cost::<WholeFrames>(&view, 1.0),
        );
    }
}

fn forward_loop(
    start_pct: f32,
    end_pct: f32,
    fade_ms: f32,
    buffer: &SampleBuffer,
) -> (super::ForwardLoop, f64) {
    let playback = crate::synth::SamplePlaybackConfig {
        loop_mode: crate::synth::SampleLoopMode::Forward,
        start_pct: 0.0,
        loop_start_pct: start_pct,
        loop_end_pct: end_pct,
        loop_fade_ms: fade_ms,
    };
    match super::SampleVoiceLoop::start(&playback, buffer) {
        (super::SampleVoiceLoop::Forward(cursor), start) => (cursor, start),
        other => panic!("expected a forward loop, got {other:?}"),
    }
}

/// The frames a whole-frame forward loop over `buffer` reads its marker frame (the only 1.0) on.
fn marker_reads(buffer: &SampleBuffer, end_pct: f32, fade_ms: f32, reads: usize) -> Vec<usize> {
    let view = SampleBufferView::from_buffer(buffer);
    let (mut cursor, mut pos) = forward_loop(0.0, end_pct, fade_ms, buffer);
    (0..reads)
        .filter(|_| cursor.read::<WholeFrames>(&view, &mut pos, 1.0).unwrap() == 1.0)
        .collect()
}

fn marked(frames: usize, marker: usize) -> SampleBuffer {
    let mut samples = vec![0.0; frames];
    samples[marker] = 1.0;
    buffer(samples)
}

#[test]
fn a_forward_loop_without_a_fade_repeats_exactly_the_loop_with_a_hard_seam() {
    let wave = |frame: usize| ((frame % 100) as f32 * std::f32::consts::TAU / 100.0).sin();
    let buffer = buffer((0..40_000).map(wave).collect());
    let view = SampleBufferView::from_buffer(&buffer);
    let (mut cursor, mut pos) = forward_loop(10.0, 60.0, 0.0, &buffer);
    for frame in 0..80_000 {
        let read = cursor.read::<WholeFrames>(&view, &mut pos, 1.0).unwrap();
        assert_eq!(read.to_bits(), wave(frame).to_bits(), "frame {frame}");
    }
    let marks = marker_reads(&marked(10_000, 5_000), 100.0, 0.0, 40_000);
    assert_eq!(marks, [5_000, 15_000, 25_000, 35_000]);
}

#[test]
fn each_forward_pass_lasts_the_loop_less_its_fade() {
    let marked = marked(10_000, 5_000);
    for (fade_ms, fade) in [(10.0, 441), (25.0, 1_103), (100.0, 4_410)] {
        let marks = marker_reads(&marked, 100.0, fade_ms, 40_000);
        let expected = (0..10)
            .map(|pass| 5_000 + pass * (10_000 - fade))
            .filter(|frame| *frame < 40_000)
            .collect::<Vec<_>>();
        assert_eq!(marks, expected, "Loop Fade {fade_ms} ms");
    }
}

fn fade_frames(frames: usize, fade_ms: f32) -> f64 {
    let (cursor, _) = forward_loop(0.0, 100.0, fade_ms, &buffer(vec![0.0; frames]));
    assert_eq!(
        cursor.jump,
        frames as f64 - (cursor.end - cursor.fade_start)
    );
    cursor.end - cursor.fade_start
}

#[test]
fn the_fade_is_at_most_half_the_loop() {
    assert_eq!(fade_frames(1_000, 10.0), 441.0);
    assert_eq!(fade_frames(1_000, 100.0), 500.0);
    assert_eq!(fade_frames(1_001, 100.0), 500.0);
    assert_eq!(fade_frames(100_000, 100.0), 4_410.0);
    assert_eq!(fade_frames(100_000, 0.0), 0.0);
}

#[test]
fn a_forward_loop_under_20_ms_plays_without_a_fade() {
    assert_eq!(fade_frames(881, 10.0), 0.0);
    assert_eq!(fade_frames(882, 10.0), 441.0);
    let at_48k = |frames: usize| SampleBuffer {
        sample_rate: 48_000,
        ..buffer(vec![0.0; frames])
    };
    for (frames, fade) in [(959, 0.0), (960, 480.0)] {
        let (cursor, _) = forward_loop(0.0, 100.0, 10.0, &at_48k(frames));
        assert_eq!(
            cursor.end - cursor.fade_start,
            fade,
            "{frames} frames at 48 kHz"
        );
    }
    let marks = marker_reads(&marked(800, 100), 100.0, 10.0, 2_400);
    assert_eq!(marks, [100, 900, 1_700]);
}

#[test]
fn a_faded_forward_seam_never_dips_bumps_or_steps() {
    let flat = buffer(vec![1.0; 10_000]);
    let view = SampleBufferView::from_buffer(&flat);
    let (mut cursor, mut pos) = forward_loop(0.0, 100.0, 10.0, &flat);
    for frame in 0..30_000 {
        let read = cursor.read::<WholeFrames>(&view, &mut pos, 1.0).unwrap();
        assert!((read - 1.0).abs() < 1.0e-6, "frame {frame}: {read}");
    }

    let wave = |frame: usize| (frame as f32 * std::f32::consts::TAU / 97.3).sin();
    let uneven = buffer((0..10_000).map(wave).collect());
    let view = SampleBufferView::from_buffer(&uneven);
    for (step, fade_ms) in [(1.0, 10.0), (1.37, 10.0), (1.37, 3.0), (0.61, 50.0)] {
        let (mut cursor, mut pos) = forward_loop(0.0, 100.0, fade_ms, &uneven);
        let reads = (0..30_000)
            .map(|_| cursor.read::<Hermite>(&view, &mut pos, step).unwrap())
            .collect::<Vec<_>>();
        let natural = std::f32::consts::TAU / 97.3 * step as f32;
        let largest = reads
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            largest < natural * 1.5,
            "step {step}, fade {fade_ms} ms: seam steps by {largest}"
        );
    }
}

#[test]
fn a_forward_loop_of_a_few_frames_plays_every_frame_at_full_level() {
    let flat = buffer(vec![1.0; 300]);
    let view = SampleBufferView::from_buffer(&flat);
    let (mut cursor, mut pos) = forward_loop(0.0, 1.0, 10.0, &flat);
    assert!((0..30).all(|_| cursor.read::<WholeFrames>(&view, &mut pos, 1.0) == Some(1.0)));
}

#[test]
fn a_held_tone_keeps_its_level_through_the_fade() {
    let wave = |frame: usize| ((frame % 79) as f32 * std::f32::consts::TAU / 79.0).sin();
    let tone = buffer((0..10_000).map(wave).collect());
    let view = SampleBufferView::from_buffer(&tone);
    let (mut cursor, mut pos) = forward_loop(0.0, 100.0, 10.0, &tone);
    assert_eq!(cursor.jump % 79.0, 0.0);
    let reads = (0..30_000)
        .map(|_| cursor.read::<WholeFrames>(&view, &mut pos, 1.0).unwrap())
        .collect::<Vec<_>>();
    let peaks = reads
        .chunks(79)
        .map(|cycle| {
            cycle
                .iter()
                .fold(0.0_f32, |peak, read| peak.max(read.abs()))
        })
        .collect::<Vec<_>>();
    let (low, high) = peaks.iter().fold((f32::MAX, 0.0_f32), |(low, high), peak| {
        (low.min(*peak), high.max(*peak))
    });
    assert!(high < 1.0 + 1.0e-3, "bumps to {high}");
    assert!(low > 1.0 - 1.0e-3, "dips to {low}");
}
