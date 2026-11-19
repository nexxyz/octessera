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
