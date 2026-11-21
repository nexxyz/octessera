use super::FxBusOutputSpreadState;
use std::f32::consts::TAU;

const SAMPLE_RATE: u32 = 48_000;

/// Left and right levels in dB after a second of `input` through full spread.
fn side_levels(input: impl Fn(usize) -> f32) -> (f32, f32) {
    let mut spread = FxBusOutputSpreadState::new(SAMPLE_RATE);
    let (mut left, mut right) = (0.0_f64, 0.0_f64);
    for frame in 0..SAMPLE_RATE as usize {
        let (l, r) = spread.process(input(frame), 1.0);
        if frame >= SAMPLE_RATE as usize / 4 {
            left += f64::from(l * l);
            right += f64::from(r * r);
        }
    }
    (10.0 * left.log10() as f32, 10.0 * right.log10() as f32)
}

fn sine(hz: f32) -> impl Fn(usize) -> f32 {
    move |frame| (frame as f32 * TAU * hz / SAMPLE_RATE as f32).sin() * 0.5
}

#[test]
fn spread_keeps_the_bass_centered() {
    for hz in [40.0, 55.0, 71.0, 100.0, 120.0] {
        let (left, right) = side_levels(sine(hz));
        assert!(
            (left - right).abs() < 1.5,
            "{hz} Hz: {left} dB vs {right} dB"
        );
    }
}

#[test]
fn spread_leans_neither_way_on_broadband_sound() {
    let mut seed = 0x1234_5678_u32;
    let noise: Vec<f32> = (0..SAMPLE_RATE as usize)
        .map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1 << 24) as f32 - 0.5
        })
        .collect();
    let (left, right) = side_levels(|frame| noise[frame]);
    assert!((left - right).abs() < 0.5, "{left} dB vs {right} dB");
}

#[test]
fn spread_still_widens_the_mids() {
    let (left, right) = side_levels(sine(1_000.0));
    assert!((left - right).abs() > 1.0, "{left} dB vs {right} dB");
}
