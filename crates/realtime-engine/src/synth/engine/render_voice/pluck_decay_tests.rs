use crate::synth::pluck_string::{PluckRing, PluckState, PluckStringSettings, RING_LEN};
use crate::synth::PluckConfig;

/// The fundamental's amplitude over `periods` whole periods from `start`, by a one-bin DFT.
fn fundamental_amplitude(samples: &[f32], start: usize, period: f64, periods: usize) -> f64 {
    let length = (period * periods as f64).round() as usize;
    let (mut re, mut im) = (0.0_f64, 0.0_f64);
    for (index, sample) in samples[start..start + length].iter().enumerate() {
        let phase = std::f64::consts::TAU * index as f64 / period;
        re += f64::from(*sample) * phase.cos();
        im += f64::from(*sample) * phase.sin();
    }
    2.0 * re.hypot(im) / length as f64
}

#[test]
fn the_fundamental_falls_sixty_db_over_decay_whatever_the_brightness() {
    let sample_rate = 44_100_u32;
    for (note, frequency, brightness_pct) in [
        (57_u8, 220.0_f32, 0.0),
        (57, 220.0, 65.0),
        (57, 220.0, 100.0),
        (81, 880.0, 65.0),
        (81, 880.0, 100.0),
    ] {
        let settings = PluckStringSettings {
            decay_ms: 1_000.0,
            brightness_pct,
            ..PluckConfig::default().into()
        };
        let mut state = PluckState::note_on(frequency, sample_rate, note, 100, settings);
        let mut ring: PluckRing = [0.0; RING_LEN];
        let samples = (0..sample_rate as usize)
            .map(|_| state.next(&mut ring))
            .collect::<Vec<_>>();
        let period = f64::from(sample_rate) / f64::from(frequency);
        let early = fundamental_amplitude(&samples, 4_410, period, 20);
        let late = fundamental_amplitude(&samples, 4_410 + 22_050, period, 20);
        let drop_db = 20.0 * (early / late).log10();
        assert!(
            (drop_db - 30.0).abs() < 2.0,
            "{frequency} Hz brightness {brightness_pct}: {drop_db} dB over half the decay"
        );
    }
}
