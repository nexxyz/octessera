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

fn pluck(frequency: f32, note: u8, brightness_pct: f32) -> Vec<f32> {
    let settings = PluckStringSettings {
        decay_ms: 1_000.0,
        brightness_pct,
        ..PluckConfig::default().into()
    };
    let mut state = PluckState::note_on(frequency, 44_100, note, 100, settings);
    let mut ring: PluckRing = [0.0; RING_LEN];
    (0..44_100).map(|_| state.next(&mut ring)).collect()
}

/// Where the brightness lowpass barely touches the fundamental, it falls 60 dB over Decay.
#[test]
fn a_low_or_bright_fundamental_falls_sixty_db_over_decay() {
    for (note, frequency, brightness_pct) in [
        (45_u8, 110.0_f32, 0.0),
        (45, 110.0, 65.0),
        (57, 220.0, 65.0),
        (57, 220.0, 100.0),
        (69, 440.0, 100.0),
    ] {
        let samples = pluck(frequency, note, brightness_pct);
        let period = 44_100.0 / f64::from(frequency);
        let early = fundamental_amplitude(&samples, 4_410, period, 20);
        let late = fundamental_amplitude(&samples, 4_410 + 22_050, period, 20);
        let drop_db = 20.0 * (early / late).log10();
        assert!(
            (drop_db - 30.0).abs() < 2.0,
            "{frequency} Hz brightness {brightness_pct}: {drop_db} dB over half the decay"
        );
    }
}

/// A dark high note, where the fundamental would need more than unity gain: the loop's DC still
/// falls at least as fast as Decay asks instead of ringing on for tens of seconds.
#[test]
fn a_dark_high_string_does_not_hold_a_dc_tail() {
    let samples = pluck(880.0, 81, 0.0);
    let mean = |start: usize| {
        let window = &samples[start..start + 4_410];
        (window.iter().map(|sample| f64::from(*sample)).sum::<f64>() / window.len() as f64).abs()
    };
    let drop_db = 20.0 * (mean(4_410) / mean(4_410 + 22_050).max(1.0e-30)).log10();
    assert!(
        drop_db > 28.0,
        "DC falls only {drop_db} dB over half the decay"
    );
}
